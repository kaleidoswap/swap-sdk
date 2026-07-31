fn main() {
    eprintln!(
        "This target only guides Maturin workspace discovery; \
         run the bindings crate's uniffi-bindgen binary instead."
    );
    std::process::exit(1);
}
