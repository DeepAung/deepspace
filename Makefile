run-server:
	cd server && bacon --job run-long
run-client:
	cargo run --bin client --features "bevy/dynamic_linking"

run-server.release:
	cargo run --bin server --release
run-client.release:
	cargo run --bin client --release
