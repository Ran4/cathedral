SHELL := /bin/sh

HOST ?= 127.0.0.1
PORT ?= 8798
UV ?= uv
FIREFOX ?= firefox
URL := http://$(HOST):$(PORT)/

.DEFAULT_GOAL := run

.PHONY: run server open

# Start the backend, wait for it to answer, open Firefox, and keep the backend
# attached to this terminal. Ctrl-C stops the server.
run:
	@set -eu; \
	if curl --fail --silent "$(URL)api/health" >/dev/null 2>&1; then \
		$(FIREFOX) --new-tab "$(URL)" >/dev/null & \
		echo "Reusing the More Sounds server already running at $(URL)"; \
		exit 0; \
	fi; \
	$(UV) run --script server.py --host "$(HOST)" --port "$(PORT)" & \
	server_pid=$$!; \
	trap 'kill $$server_pid 2>/dev/null || true' INT TERM EXIT; \
	attempt=0; \
	until curl --fail --silent --show-error "$(URL)api/health" >/dev/null 2>&1; do \
		if ! kill -0 $$server_pid 2>/dev/null; then \
			set +e; \
			wait $$server_pid; \
			status=$$?; \
			set -e; \
			echo "If another program owns port $(PORT), retry with: make PORT=$$(( $(PORT) + 1 ))" >&2; \
			exit $$status; \
		fi; \
		attempt=$$((attempt + 1)); \
		if [ $$attempt -ge 300 ]; then \
			echo "Timed out waiting for $(URL)" >&2; \
			exit 1; \
		fi; \
		sleep 0.1; \
	done; \
	$(FIREFOX) --new-tab "$(URL)" >/dev/null & \
	echo "More Sounds workbench: $(URL) (Ctrl-C to stop)"; \
	wait $$server_pid

# Run only the backend.
server:
	$(UV) run --script server.py --host "$(HOST)" --port "$(PORT)"

# Open an already-running backend in Firefox.
open:
	$(FIREFOX) --new-tab "$(URL)"
