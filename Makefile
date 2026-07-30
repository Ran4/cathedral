.PHONY: run features-ui

run:
	cargo run --release

features-ui:
	@if curl -sf -m 1 -o /dev/null http://127.0.0.1:8123/api/features; then \
		echo "already running, opening browser"; \
		firefox http://127.0.0.1:8123/ >/dev/null 2>&1 & \
	else \
		( sleep 1 && firefox http://127.0.0.1:8123/ >/dev/null 2>&1 ) & \
		uv run features/ui/server.py; \
	fi
