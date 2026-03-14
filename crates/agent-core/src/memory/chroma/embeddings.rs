use crate::memory::retrieval::extract_terms;

pub(super) fn chroma_disabled_via_env() -> bool {
    matches!(
        std::env::var("CHROMA_DISABLED").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

pub(super) fn embed_text(input: &str) -> Vec<f32> {
    const DIMENSIONS: usize = 128;

    let mut vector = vec![0.0_f32; DIMENSIONS];
    for term in extract_terms(input) {
        let hash = stable_hash(&term);
        let index = (hash as usize) % DIMENSIONS;
        let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
        vector[index] += sign;
    }

    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }

    vector
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
