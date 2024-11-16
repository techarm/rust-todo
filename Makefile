build:
	docker compose build

db:
	docker compose up -d

dev:
	cargo watch -x run

test:
	cargo test