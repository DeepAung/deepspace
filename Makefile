run-server:
	cd server && bacon --job run-long
run-client:
	cargo run --bin client --features "bevy/dynamic_linking"
