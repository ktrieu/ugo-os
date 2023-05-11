fn main() {
    println!(
        "cargo:rustc-link-arg=--image-base=0x{:016x}",
        common::KERNEL_START
    )
}
