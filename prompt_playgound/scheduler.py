"""Non-blocking, sequential round-robin scheduling for LLM characters."""

from __future__ import annotations

import queue
import sys
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass

from prompt import parse_reply, render_prompt_and_drain
from sim import CharIdStr, World, apply_action

MAX_LLM_REPLY_CHARS = 100_000


@dataclass(frozen=True, slots=True)
class SchedulerStatus:
    subsystem: str
    state: str
    actor_id: CharIdStr | None = None
    message: str | None = None

    def to_payload(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "subsystem": self.subsystem,
            "state": self.state,
        }
        if self.actor_id is not None:
            payload["actor_id"] = str(self.actor_id)
        if self.message is not None:
            payload["message"] = self.message
        return payload


@dataclass(frozen=True, slots=True)
class _CompletionResult:
    actor_id: CharIdStr
    reply: str | None
    error: Exception | None


class _CompletionWorker:
    """One daemon provider worker; it cannot hold server shutdown hostage."""

    def __init__(self, complete: Callable[[str], str]) -> None:
        self._complete = complete
        self._requests: queue.Queue[tuple[CharIdStr, str] | None] = queue.Queue(
            maxsize=1
        )
        self._results: queue.SimpleQueue[_CompletionResult] = queue.SimpleQueue()
        self._thread = threading.Thread(
            target=self._run, name="smart-actor-llm", daemon=True
        )
        self._thread.start()

    def submit(self, actor_id: CharIdStr, prompt: str) -> None:
        self._requests.put_nowait((actor_id, prompt))

    def poll(self) -> _CompletionResult | None:
        try:
            return self._results.get_nowait()
        except queue.Empty:
            return None

    def close(self) -> None:
        try:
            self._requests.put_nowait(None)
        except queue.Full:
            pass

    def _run(self) -> None:
        while True:
            task = self._requests.get()
            if task is None:
                return
            actor_id, prompt = task
            try:
                reply = self._complete(prompt)
                if not isinstance(reply, str):
                    raise TypeError("LLM completion must return text")
                if len(reply) > MAX_LLM_REPLY_CHARS:
                    raise ValueError("LLM reply exceeded the service size limit")
            except Exception as error:
                self._results.put(_CompletionResult(actor_id, None, error))
            else:
                self._results.put(_CompletionResult(actor_id, reply, None))


class NpcScheduler:
    """Drive a global NPC turn stream without blocking the state thread.

    ``poll`` must be called by the protocol/state thread. Only the provider call
    runs on the daemon worker; replies are parsed and actions are revalidated on
    the next ``poll`` against the latest world.
    """

    def __init__(
        self,
        world: World,
        complete: Callable[[str], str],
        *,
        minimum_delay_seconds: float = 1.0,
        maximum_backoff_seconds: float = 60.0,
        clock: Callable[[], float] = time.monotonic,
        verbose: bool = False,
    ) -> None:
        if minimum_delay_seconds < 0:
            raise ValueError("minimum_delay_seconds cannot be negative")
        self.world = world
        self.minimum_delay_seconds = minimum_delay_seconds
        self.maximum_backoff_seconds = max(
            1.0, maximum_backoff_seconds, minimum_delay_seconds
        )
        self._clock = clock
        self._verbose = verbose
        self._worker = _CompletionWorker(complete)
        self._order = [
            actor.id for actor in world.characters.values() if actor.control == "llm"
        ]
        self._round_robin_index = 0
        self._priority_actor_id: CharIdStr | None = None
        self._in_flight_actor_id: CharIdStr | None = None
        self._in_flight_events: list[str] = []
        self._next_turn_at = self._clock()
        self._provider_failures = 0
        self._running = False

    @property
    def in_flight_actor_id(self) -> CharIdStr | None:
        return self._in_flight_actor_id

    @property
    def running(self) -> bool:
        return self._running

    def start(self) -> None:
        self._running = True
        self._next_turn_at = min(self._next_turn_at, self._clock())

    def close(self) -> None:
        self._running = False
        self._worker.close()

    def prioritize(self, actor_id: CharIdStr | str) -> bool:
        actor_id = CharIdStr(str(actor_id))
        actor = self.world.characters.get(actor_id)
        if actor is None or actor.control != "llm":
            return False
        self._priority_actor_id = actor_id
        return True

    def poll(self, now: float | None = None) -> list[SchedulerStatus]:
        now = self._clock() if now is None else now
        statuses: list[SchedulerStatus] = []

        result = self._worker.poll() if self._in_flight_actor_id is not None else None
        if result is not None:
            # Discard a result whose actor disappeared or which somehow does not
            # correspond to the sole in-flight request.
            expected = self._in_flight_actor_id
            self._in_flight_actor_id = None
            actor = self.world.characters.get(result.actor_id)
            if result.actor_id != expected or actor is None or actor.control != "llm":
                statuses.append(
                    SchedulerStatus(
                        "llm", "degraded", message="discarded a stale LLM result"
                    )
                )
            elif result.error is not None:
                # The provider never produced a turn, so let the actor perceive
                # the events that were moved into that failed prompt again. New
                # events collected during the request remain after them.
                actor.inbox = self._in_flight_events + actor.inbox
                self._provider_failures += 1
                backoff = min(
                    self.maximum_backoff_seconds,
                    self.minimum_delay_seconds * (2 ** (self._provider_failures - 1)),
                )
                # A zero development delay still needs a non-spinning failure delay.
                backoff = max(backoff, min(1.0, self.maximum_backoff_seconds))
                self._next_turn_at = now + backoff
                actor.inbox.append(
                    "system: the cognition provider failed; your turn will be retried later"
                )
                print(
                    f"[smart actors] LLM request for {actor.name} failed: "
                    f"{type(result.error).__name__}",
                    file=sys.stderr,
                )
                statuses.append(
                    SchedulerStatus(
                        "llm",
                        "degraded",
                        actor.id,
                        f"provider request failed; retrying in {backoff:g} seconds",
                    )
                )
            else:
                self._provider_failures = 0
                self._next_turn_at = now + self.minimum_delay_seconds
                if self._verbose:
                    print(
                        f"--- reply from {actor.name} ---\n{result.reply}",
                        file=sys.stderr,
                    )
                actions, errors = parse_reply(result.reply)
                for error in errors:
                    actor.inbox.append(f"system: your last output was invalid: {error}")
                    print(f"[smart actors] {actor.name}: {error}", file=sys.stderr)
                for verb, action_args in actions:
                    try:
                        line = apply_action(self.world, actor, verb, action_args)
                    except Exception as error:
                        # apply_action is designed to raise ActionError, but this
                        # final boundary ensures arbitrary model output never
                        # takes down the process even if a future verb regresses.
                        actor.inbox.append(
                            f'system: your action "{verb} {action_args}" failed: {error}'
                        )
                        print(
                            f"[smart actors] {actor.name}: {verb} failed: {error}",
                            file=sys.stderr,
                        )
                        continue
                    # Waiting is a real, validated model choice but not a world
                    # event and should not make the transcript grow forever.
                    if verb != "wait":
                        self.world.transcript.append(line)
                    if self._verbose and verb != "wait":
                        print(f"[smart actors] {line}", file=sys.stderr)
                statuses.append(SchedulerStatus("llm", "idle", actor.id))
            self._in_flight_events = []

        if (
            self._running
            and self._in_flight_actor_id is None
            and self._order
            and now >= self._next_turn_at
        ):
            actor_id = self._select_next_actor()
            actor = self.world.characters.get(actor_id)
            if actor is not None and actor.control == "llm":
                drained_events = list(actor.inbox)
                try:
                    prompt = render_prompt_and_drain(self.world, actor)
                    if self._verbose:
                        print(
                            f"--- prompt for {actor.name} ---\n{prompt}",
                            file=sys.stderr,
                        )
                except Exception as error:
                    actor.inbox.append("system: your prompt could not be prepared")
                    self._next_turn_at = now + max(self.minimum_delay_seconds, 1.0)
                    print(
                        f"[smart actors] prompt for {actor.name} failed: {error}",
                        file=sys.stderr,
                    )
                    statuses.append(
                        SchedulerStatus(
                            "llm", "degraded", actor.id, "prompt rendering failed"
                        )
                    )
                else:
                    try:
                        self._worker.submit(actor.id, prompt)
                    except Exception as error:
                        actor.inbox = drained_events + actor.inbox
                        actor.inbox.append("system: the cognition worker is busy")
                        self._next_turn_at = now + max(self.minimum_delay_seconds, 1.0)
                        print(
                            f"[smart actors] could not queue {actor.name}'s turn: {error}",
                            file=sys.stderr,
                        )
                        statuses.append(
                            SchedulerStatus(
                                "llm", "degraded", actor.id, "cognition worker is busy"
                            )
                        )
                    else:
                        self._in_flight_actor_id = actor.id
                        self._in_flight_events = drained_events
                        statuses.append(SchedulerStatus("llm", "thinking", actor.id))

        return statuses

    def _select_next_actor(self) -> CharIdStr:
        if self._priority_actor_id is not None:
            actor_id = self._priority_actor_id
            self._priority_actor_id = None
            return actor_id
        actor_id = self._order[self._round_robin_index % len(self._order)]
        self._round_robin_index = (self._round_robin_index + 1) % len(self._order)
        return actor_id
