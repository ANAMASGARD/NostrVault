/// A foundation probe only; no protocol, cryptography, or storage policy.
#[no_mangle]
pub extern "C" fn runtime_probe(input: u32) -> u32 {
    input.wrapping_mul(2).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::runtime_probe;

    #[test]
    fn probe_has_defined_u32_overflow_behavior() {
        assert_eq!(runtime_probe(42), 85);
        assert_eq!(runtime_probe(u32::MAX), u32::MAX);
    }
}
