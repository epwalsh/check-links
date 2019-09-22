.PHONY : build
build :
	cargo build --verbose

.PHONY : fmt
fmt :
	cargo fmt --

.PHONY : check-fmt
check-fmt :
	cargo fmt -- --check

.PHONY : test
test :
	cargo test --verbose
