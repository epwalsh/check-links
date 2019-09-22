.PHONY : build
build :
	cargo build --verbose

.PHONY : fmt
fmt :
	cargo fmt -- --check

.PHONY : test
test :
	cargo test --verbose
