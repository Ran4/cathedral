Status: Planned (2026-09-05).

# M10 — Clear conversation and controls

Make the player understand the current situation without freezing the city or solving the mystery for them. Natural language and explicit controls invoke the same actions.

## Entry

M1–M9 are accepted. Extend the shipped knowledge journal and existing chat/inventory interaction patterns rather than adding an independent quest journal with its own truth.

## The player-facing contract

Every consequential exchange distinguishes intention, request, acceptance, progress and result. Use short specific language: “Requested an inspection”, “Lise agreed to the loft only”, “The door is opening”, “The comparison was interrupted”, “The papers were lodged”. A request to arrest somebody does not display “Arrested”.

A compact activity/status view shows the next known appointment, pending accepted work and actual important changes. It draws from the player's own receipts and authorised records, not hidden actor transforms or internal task queues.

## Conversation and action selection

Natural speech/typing can identify a learned statement or visible object and propose an action. Resolve ambiguous people, objects and claims through a compact picker/confirmation. Ambiguous or mis-transcribed language cannot confirm a formal accusation, lodge an account, accept an undertaking or transfer property without the player reviewing the resolved meaning and intentionally committing it. Ordinary conversation remains audible and can produce reactions or rumours before confirmation. Correcting or cancelling the proposed action does not erase words already heard.

Person/object/claim pickers and language resolution use the notebook's learned and authorised projection. A guessed name, typed claim or opaque ID cannot enumerate hidden records, identify an unseen object, authenticate a statement or reveal current custody. Preserve unsupported reconstructions as hypotheses or attributed allegations. Opening or editing a draft creates no disclosure event; its audience label expresses intended delivery, not achieved confidentiality.

Keep the ordinary microphone and chat purpose explicit. If an account field offers voice dictation, visibly identify it as unsent drafting and bind that purpose when recording starts. Preserve purpose, draft/proposition version, learned references and interruption status across delayed callbacks and save/load. Opening a notebook must not silently redirect ordinary speech. A late transcript cannot turn cancelled or restored dictation into public chat. This is a local input-field contract, not a global quest or voice mode.

The explicit action list exposes the same available interactions with their current scope. Examples: ask about a learned claim, request limited entry, show/compare an object, confirm an account, lodge a selected bundle, invite a stand-in and request a finding.

The sim revalidates when the command commits. A moving recipient, revoked grant or changed item produces a specific receipt and preserves the draft. Avoid making unrelated world revisions invalidate every selection.

Recorded accounts use a stable core proposition. Show what is being put on record and by whom; model phrasing must not silently change its semantic content. Free conversation stays available around this, including evasions and refusals.

## Notebook and submission views

Arrange learned accounts, places/routes, objects/custody and personal reconstruction within the shared journal. Preserve sources, uncertainty, previous versions and last confirmed locations. Let the player attach known observations to a hypothesis without turning it into an established fact.

Use bounded learned-record pages/deltas with a request/revision contract. Keep private history out of `PublicSnapshot`, preserve the existing prompt/authored-cast snapshot canaries and measure bytes plus publication frequency. Unrelated crowd changes must not retransmit an entire archive or invalidate every page. See [RUNTIME_BUDGETS](RUNTIME_BUDGETS.md).

Before lodging a bundle, show the requested finding, selected support, disclosure audience and established admissibility issues. Sealed delivery grants access to the record without speaking its proposition. Spoken presentation and confirmation follow ordinary hearing; show who is addressed and that others may hear, without certifying an exhaustive audience. Record privacy cannot retroactively make already public speech secret.

A privacy condition change perceived by the player or explicitly reported by a participant may interrupt an as-yet-unemitted controlled presentation, retaining its draft. An undetected listener alone cannot change warnings, action availability or interruption outcomes, even anonymously. Their authoritative hearing receipt may differ and later support learned consequences. Preserve already committed speech when a pending formal action is interrupted.

Audit transcript delivery badges as well as notebook content. Replace the current authoritative-recipient count and “nobody nearby” feedback with learned delivery or acknowledgement information. The interface cannot certify an unseen audience's size or absence.

The view may say “These two accounts come from the same original witness” when the shared chain has been established. An unlearned secret retelling cannot appear as an omniscient defect. The view may not name a missing unknown fragment or automatically select the culprit. There is no guilt percentage or mandatory trail of highlighted clues.

## Never-pause interaction design

- Reading panels remain dismissible and retain position; important local events can be noticed without losing the document.
- Drafts survive interruptions, target departure and load where supported.
- Appointments show calendar time, known location and known status. If an off-stage scheduled time passed without a received outcome, show “Scheduled time passed; outcome unconfirmed.” Underway, held, postponed and cancelled require actual learned receipts.
- Event notices explain actual consequences: a missed examination, moved evidence known through a report, an amended hearing result. They do not infer unknown events.
- Long provider latency cannot consume an essential one-shot opportunity with no fallback. Deterministic action controls and later recorded accounts remain available.
- A participant with urgent duties can end a conversation with a clear reason and next way to reach them.
- Accessibility uses readable text, captions, clear focus, keyboard controls, contrast and scalable UI; it does not introduce a hidden pause.

## Hints

Use layered reminders grounded in learned material: the last established result, a known unresolved question, a known source who could clarify it, then a restatement of an already learned contradiction. Do not reveal the upper route or an object's hidden location before any observation/source makes it available.

## Tests and play evidence

- Voice, typing and explicit controls use equivalent semantic action/results for the same confirmed action and disclosure channel. Freely spoken input retains its additional hearing effects; unsent drafting does not acquire them merely to make inputs look identical.
- Ambiguous names, poor transcription and stale targets cannot commit an unintended formal accusation or transfer. A correction preserves the original ordinary speech receipt and any established reactions.
- A slow reader spends several minutes in each panel while the city advances and later resumes coherently.
- The player can identify what is pending, completed, interrupted and merely alleged from the interface alone.
- Public/private submission tests verify actual fact/statement recipients, not just different button labels.
- A perceived door opening interrupts an unemitted sensitive presentation. Paired worlds differing only by an undetected listener retain identical privacy UI and submission outcomes, although actual hearing receipts can differ. Sealed delivery emits no proposition speech.
- Hidden identity, custody and author-truth changes alone cannot change picker enumeration, claim resolution or unavailable-action explanations. Recipient badges reveal only learned delivery/acknowledgements.
- Cancelled drafting, delayed transcription and restore produce neither public-chat fallback nor duplicate speech/filing. Input purpose remains bound to the original recording.
- The player reads across a review time while the reviewer is blocked elsewhere; the notebook reports an unconfirmed outcome until an actual report/docket inspection updates it.
- Notebook and save-slot previews never expose hidden custody, suspect location or author truth.
- Screen/keyboard scaling and captions work in hidden-window drive captures; actual readability and comprehension need human sessions.

## Completion gate

The independent development disputes can be played through ordinary conversation and explicit controls. Players can explain what actions happened and what evidence supports their view. M16/M17 can author dramatic encounters using these surfaces without adding a special quest mode.
