#![feature(test)]
extern crate test;

extern crate rand;
extern crate rand_core;

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    use ff::Field;
    use group::{CurveAffine, CurveProjective, EncodedPoint};
    use pairing::bls12_381;
    use pairing::bls12_381::Fr;
    use pairing::bls12_381::{G1Compressed, G1Uncompressed};

    fn generate_uncompressed_random_element() -> G1Uncompressed {
        let mut random_gen = rand::thread_rng();
        let random_g1 = bls12_381::G1::random(&mut random_gen);
        let uncompressed_form = bls12_381::G1Uncompressed::from_affine(random_g1.into_affine());
        uncompressed_form
    }

    fn generate_compressed_random_element() -> G1Compressed {
        let mut random_gen = rand::thread_rng();
        let random_g1 = bls12_381::G1::random(&mut random_gen);
        let compressed_form = bls12_381::G1Compressed::from_affine(random_g1.into_affine());
        compressed_form
    }

    fn generate_random_fr_element() -> Fr {
        let mut random_gen = rand::thread_rng();
        Fr::random(&mut random_gen)
    }
    // --------------------- G1 Uncompressed ---------------------------
    #[bench]
    fn bench_uncompressed_generate_zero(b: &mut Bencher) {
        b.iter(|| {
            let zero = bls12_381::G1Affine::zero();
            let uncompressed_form = bls12_381::G1Uncompressed::from_affine(zero);
            uncompressed_form
        })
    }

    #[bench]
    fn bench_uncompressed_generate_one(b: &mut Bencher) {
        b.iter(|| {
            let r = bls12_381::G1Affine::one();
            let uncompressed_form = bls12_381::G1Uncompressed::from_affine(r);
            uncompressed_form
        })
    }

    #[bench]
    fn bench_uncompressed_generate_random(b: &mut Bencher) {
        b.iter(|| generate_uncompressed_random_element())
    }

    #[bench]
    fn bench_uncompressed_check_if_zero_on_pregenerated_zero(b: &mut Bencher) {
        let g = bls12_381::G1Uncompressed::from_affine(bls12_381::G1Affine::zero());

        b.iter(|| {
            let g = g.into_affine_unchecked().unwrap();
            g.is_zero()
        })
    }

    #[bench]
    fn bench_uncompressed_check_if_zero_on_pregenerated_one(b: &mut Bencher) {
        let g = bls12_381::G1Uncompressed::from_affine(bls12_381::G1Affine::one());

        b.iter(|| {
            let g = g.into_affine_unchecked().unwrap();
            g.is_zero()
        })
    }

    #[bench]
    fn bench_uncompressed_check_if_zero_on_pregenerated_random_element(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();

        b.iter(|| {
            let g = g.into_affine_unchecked().unwrap();
            g.is_zero()
        })
    }

    #[bench]
    fn bench_uncompressed_add_unchecked_conversion_from_uncompressed_to_affine(b: &mut Bencher) {
        let g1 = generate_uncompressed_random_element();
        let g2 = generate_uncompressed_random_element();
        b.iter(|| {
            let mut g1 = g1.into_affine_unchecked().unwrap().into_projective();
            let g2 = g2.into_affine_unchecked().unwrap().into_projective();
            g1.add_assign(&g2);
            bls12_381::G1Uncompressed::from_affine(g1.into_affine())
        })
    }

    #[bench]
    fn bench_uncompressed_add_checked_conversion_from_uncompressed_to_affine(b: &mut Bencher) {
        let g1 = generate_uncompressed_random_element();
        let g2 = generate_uncompressed_random_element();
        b.iter(|| {
            let mut g1 = g1.into_affine().unwrap().into_projective();
            let g2 = g2.into_affine().unwrap().into_projective();
            g1.add_assign(&g2);
            bls12_381::G1Uncompressed::from_affine(g1.into_affine())
        })
    }

    #[bench]
    fn bench_uncompressed_opposite_of_pregenerated_random_element(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap();
            g.negate();
            bls12_381::G1Uncompressed::from_affine(g)
        })
    }

    #[bench]
    fn bench_uncompressed_negate_of_pregenerated_zero_element(b: &mut Bencher) {
        let g = bls12_381::G1Uncompressed::from_affine(bls12_381::G1Affine::zero());

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap();
            g.negate();
            bls12_381::G1Uncompressed::from_affine(g)
        })
    }

    #[bench]
    fn bench_uncompressed_negate_of_pregenerated_one_element(b: &mut Bencher) {
        let g = bls12_381::G1Uncompressed::from_affine(bls12_381::G1Affine::one());

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap();
            g.negate();
            bls12_381::G1Uncompressed::from_affine(g)
        })
    }

    #[bench]
    fn bench_uncompressed_eq_with_pregenerated_random_elements(b: &mut Bencher) {
        let g1 = generate_uncompressed_random_element();
        let g2 = generate_uncompressed_random_element();

        b.iter(|| {
            let g1 = g1.into_affine_unchecked().unwrap();
            let g2 = g2.into_affine_unchecked().unwrap();
            g1 == g2
        })
    }

    #[bench]
    fn bench_uncompressed_eq_with_pregenerated_random_element_and_zero(b: &mut Bencher) {
        let g1 = generate_uncompressed_random_element();
        let g2 = bls12_381::G1Uncompressed::from_affine(bls12_381::G1Affine::zero());

        b.iter(|| {
            let g1 = g1.into_affine_unchecked().unwrap();
            let g2 = g2.into_affine_unchecked().unwrap();
            g1 == g2
        })
    }

    #[bench]
    fn bench_uncompressed_eq_with_pregenerated_random_element_and_one(b: &mut Bencher) {
        let g1 = generate_uncompressed_random_element();
        let g2 = bls12_381::G1Uncompressed::from_affine(bls12_381::G1Affine::one());

        b.iter(|| {
            let g1 = g1.into_affine_unchecked().unwrap();
            let g2 = g2.into_affine_unchecked().unwrap();
            g1 == g2
        })
    }

    #[bench]
    fn bench_uncompressed_mul_with_pregenerated_random_elements(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();
        let a = generate_random_fr_element();

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap().into_projective();
            g.mul_assign(a);
            bls12_381::G1Uncompressed::from_affine(g.into_affine());
        })
    }

    #[bench]
    fn bench_from_uncompressed_to_projective_with_unchecked_affine_conversion(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();
        b.iter(|| {
            g.into_affine_unchecked().unwrap().into_projective();
        })
    }

    #[bench]
    fn bench_from_uncompressed_to_projective_with_checked_affine_conversion(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();
        b.iter(|| {
            g.into_affine().unwrap().into_projective();
        })
    }

    #[bench]
    fn bench_from_uncompressed_to_affine(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();
        b.iter(|| {
            g.into_affine().unwrap();
        })
    }

    #[bench]
    fn bench_from_uncompressed_to_unchecked_affine(b: &mut Bencher) {
        let g = generate_uncompressed_random_element();
        b.iter(|| {
            g.into_affine_unchecked().unwrap();
        })
    }

    #[bench]
    fn bench_from_affine_to_projective(b: &mut Bencher) {
        let g = generate_uncompressed_random_element()
            .into_affine()
            .unwrap();
        b.iter(|| {
            g.into_projective();
        })
    }

    #[bench]
    fn bench_from_projective_to_affine(b: &mut Bencher) {
        let g = generate_uncompressed_random_element()
            .into_affine()
            .unwrap()
            .into_projective();
        b.iter(|| {
            g.into_affine();
        })
    }

    // --------------------- G1 Compressed ---------------------------
    #[bench]
    fn bench_compressed_add(b: &mut Bencher) {
        let g1 = generate_compressed_random_element();
        let g2 = generate_compressed_random_element();
        b.iter(|| {
            // First we must convert in projective, add, and convert back into
            // affine to get the uncompressed form.
            let mut g1 = g1.into_affine_unchecked().unwrap().into_projective();
            let g2 = g2.into_affine_unchecked().unwrap().into_projective();
            g1.add_assign(&g2);
            bls12_381::G1Compressed::from_affine(g1.into_affine())
        })
    }

    #[bench]
    fn bench_compressed_opposite_of_pregenerated_random_element(b: &mut Bencher) {
        let g = generate_compressed_random_element();

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap();
            g.negate();
            bls12_381::G1Compressed::from_affine(g)
        })
    }

    #[bench]
    fn bench_compressed_negate_of_pregenerated_zero_element(b: &mut Bencher) {
        let g = bls12_381::G1Compressed::from_affine(bls12_381::G1Affine::zero());

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap();
            g.negate();
            bls12_381::G1Compressed::from_affine(g)
        })
    }

    #[bench]
    fn bench_compressed_negate_of_pregenerated_one_element(b: &mut Bencher) {
        let g = bls12_381::G1Compressed::from_affine(bls12_381::G1Affine::one());

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap();
            g.negate();
            bls12_381::G1Compressed::from_affine(g)
        })
    }

    #[bench]
    fn bench_compressed_check_if_zero_on_pregenerated_zero(b: &mut Bencher) {
        let g = bls12_381::G1Compressed::from_affine(bls12_381::G1Affine::zero());

        b.iter(|| {
            let g = g.into_affine_unchecked().unwrap();
            g.is_zero()
        })
    }

    #[bench]
    fn bench_compressed_check_if_zero_on_pregenerated_one(b: &mut Bencher) {
        let g = bls12_381::G1Compressed::from_affine(bls12_381::G1Affine::one());

        b.iter(|| {
            let g = g.into_affine_unchecked().unwrap();
            g.is_zero()
        })
    }

    #[bench]
    fn bench_compressed_check_if_zero_on_pregenerated_random_element(b: &mut Bencher) {
        let g = generate_compressed_random_element();

        b.iter(|| {
            let g = g.into_affine_unchecked().unwrap();
            g.is_zero()
        })
    }

    #[bench]
    fn bench_compressed_eq_with_pregenerated_random_elements(b: &mut Bencher) {
        let g1 = generate_compressed_random_element();
        let g2 = generate_compressed_random_element();

        b.iter(|| {
            let g1 = g1.into_affine_unchecked().unwrap();
            let g2 = g2.into_affine_unchecked().unwrap();
            g1 == g2
        })
    }

    #[bench]
    fn bench_compressed_eq_with_pregenerated_random_element_and_zero(b: &mut Bencher) {
        let g1 = generate_compressed_random_element();
        let g2 = bls12_381::G1Compressed::from_affine(bls12_381::G1Affine::zero());

        b.iter(|| {
            let g1 = g1.into_affine_unchecked().unwrap();
            let g2 = g2.into_affine_unchecked().unwrap();
            g1 == g2
        })
    }

    #[bench]
    fn bench_compressed_eq_with_pregenerated_random_element_and_one(b: &mut Bencher) {
        let g1 = generate_compressed_random_element();
        let g2 = bls12_381::G1Compressed::from_affine(bls12_381::G1Affine::one());

        b.iter(|| {
            let g1 = g1.into_affine_unchecked().unwrap();
            let g2 = g2.into_affine_unchecked().unwrap();
            g1 == g2
        })
    }

    #[bench]
    fn bench_compressed_mul_with_pregenerated_random_elements(b: &mut Bencher) {
        let g = generate_compressed_random_element();
        let a = generate_random_fr_element();

        b.iter(|| {
            let mut g = g.into_affine_unchecked().unwrap().into_projective();
            g.mul_assign(a);
            bls12_381::G1Compressed::from_affine(g.into_affine());
        })
    }
}
