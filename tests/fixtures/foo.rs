//! Only links in doc comments should be checks like this: [README.md](README.md)

/// and this: [README.md](README.md)
struct Foo {}

// but not this [README.md](foo.md)
