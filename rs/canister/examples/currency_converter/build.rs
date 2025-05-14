fn main() {
    // Rebuild if canister DID changes
    println!("cargo:rerun-if-changed=can.did");
} 