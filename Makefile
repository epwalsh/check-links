.PHONY : release
release :
	cargo build --release

.PHONY : build
build :
	cargo build

.PHONY : fmt
fmt :
	cargo fmt --

.PHONY : check-fmt
check-fmt :
	cargo fmt -- --check

.PHONY : test
test :
	cargo test --verbose

.PHONY : clippy
clippy :
	cargo clippy -- -D warnings
