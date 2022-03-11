test:
	export RUST_BACKTRACE=1
	./build.sh
	cargo test -- --nocapture
