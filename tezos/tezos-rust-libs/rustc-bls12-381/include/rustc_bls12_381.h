#ifndef RUSTC_BLS12_381_INCLUDE_H
#define RUSTC_BLS12_381_INCLUDE_H
#include <stdbool.h>


// Fq12
extern bool rustc_bls12_381_fq12_check_bytes(const unsigned char *element);

extern bool rustc_bls12_381_fq12_is_zero(const unsigned char *x);

extern bool rustc_bls12_381_fq12_is_one(const unsigned char *x);

extern void rustc_bls12_381_fq12_random(unsigned char *buffer);

extern void rustc_bls12_381_fq12_one(unsigned char *buffer);

extern void rustc_bls12_381_fq12_zero(unsigned char *buffer);

extern void rustc_bls12_381_fq12_add(unsigned char *buffer,
                                     const unsigned char *x,
                                     const unsigned char *y);
extern bool rustc_bls12_381_fq12_eq(const unsigned char *x,
                                    const unsigned char *y);

extern void rustc_bls12_381_fq12_mul(unsigned char *buffer,
                                     const unsigned char *x,
                                     const unsigned char *y);

extern void rustc_bls12_381_fq12_unsafe_inverse(unsigned char *buffer,
                                                const unsigned char *x);

extern void rustc_bls12_381_fq12_negate(unsigned char *buffer,
                                        const unsigned char *x);

extern void rustc_bls12_381_fq12_square(unsigned char *buffer,
                                        const unsigned char *x);

extern void rustc_bls12_381_fq12_double(unsigned char *buffer,
                                        const unsigned char *x);

extern void rustc_bls12_381_fq12_pow(unsigned char *buffer,
                                     const unsigned char *x,
                                     const unsigned char *n);

// Fr
extern bool rustc_bls12_381_fr_check_bytes(const unsigned char *element);

extern bool rustc_bls12_381_fr_is_zero(const unsigned char *element);

extern bool rustc_bls12_381_fr_is_one(const unsigned char *x);

extern void rustc_bls12_381_fr_zero(unsigned char *buffer);

extern void rustc_bls12_381_fr_one(unsigned char *buffer);

extern void rustc_bls12_381_fr_random(unsigned char *buffer);

extern void rustc_bls12_381_fr_add(unsigned char *buffer,
                                   const unsigned char *x,
                                   const unsigned char *y);

extern void rustc_bls12_381_fr_mul(unsigned char *buffer,
                                   const unsigned char *x,
                                   const unsigned char *y);

extern void rustc_bls12_381_fr_unsafe_inverse(unsigned char *buffer,
                                              const unsigned char *x);

extern void rustc_bls12_381_fr_negate(unsigned char *buffer,
                                      const unsigned char *x);

extern bool rustc_bls12_381_fr_eq(const unsigned char *x,
                                  const unsigned char *y);

extern void rustc_bls12_381_fr_square(unsigned char *buffer,
                                      const unsigned char *x);

extern void rustc_bls12_381_fr_double(unsigned char *buffer,
                                      const unsigned char *x);

extern void rustc_bls12_381_fr_pow(unsigned char *buffer,
                                   const unsigned char *x,
                                   const unsigned char *n);

// G1 uncompressed
extern bool rustc_bls12_381_g1_uncompressed_check_bytes(const unsigned char *element);

extern bool rustc_bls12_381_g1_compressed_check_bytes(const unsigned char *element);

extern void rustc_bls12_381_g1_one(unsigned char *buffer);

extern void rustc_bls12_381_g1_zero(unsigned char *buffer);

extern void rustc_bls12_381_g1_random(unsigned char *element);

extern void rustc_bls12_381_g1_add(unsigned char *buffer,
                                   const unsigned char* g1,
                                   const unsigned char *g2);

extern void rustc_bls12_381_g1_negate(unsigned char *buffer,
                                      const unsigned char *g);

extern bool rustc_bls12_381_g1_eq(const unsigned char *g1,
                                  const unsigned char *g2);

extern bool rustc_bls12_381_g1_is_zero(const unsigned char *g);

extern void rustc_bls12_381_g1_double(unsigned char *buffer,
                                      const unsigned char *g);

extern void rustc_bls12_381_g1_mul(unsigned char *buffer,
                                   const unsigned char *g,
                                   const unsigned char *a);

// ---------------- G1 Compressed ------------
extern void rustc_bls12_381_g1_compressed_add(unsigned char *buffer,
                                              const unsigned char* g1,
                                              const unsigned char *g2);

extern void rustc_bls12_381_g1_compressed_negate(unsigned char *buffer,
                                                 const unsigned char *g);

extern bool rustc_bls12_381_g1_compressed_eq(const unsigned char *g1,
                                             const unsigned char *g2);

extern bool rustc_bls12_381_g1_compressed_is_zero(const unsigned char *g);

extern void rustc_bls12_381_g1_compressed_of_uncompressed(unsigned char *buffer,
                                                          const unsigned char *g);

extern void
rustc_bls12_381_g1_uncompressed_of_compressed(unsigned char *buffer,
                                              const unsigned char *g);

extern void rustc_bls12_381_g1_compressed_one(unsigned char *buffer);

extern void rustc_bls12_381_g1_compressed_zero(unsigned char *buffer);

extern void rustc_bls12_381_g1_compressed_random(unsigned char *element);

extern void rustc_bls12_381_g1_compressed_double(unsigned char *buffer,
                                                 const unsigned char *g);

extern void rustc_bls12_381_g1_compressed_mul(unsigned char *buffer,
                                              const unsigned char *g,
                                              const unsigned char *a);

extern bool rustc_bls12_381_g1_build_from_components(unsigned char *buffer,
                                                     const unsigned char *x,
                                                     const unsigned char *y);

// G2 uncompressed
extern bool rustc_bls12_381_g2_uncompressed_check_bytes(const unsigned char *element);

extern bool rustc_bls12_381_g2_compressed_check_bytes(const unsigned char *element);

extern void rustc_bls12_381_g2_one(unsigned char *buffer);

extern void rustc_bls12_381_g2_zero(unsigned char *buffer);

extern void rustc_bls12_381_g2_random(unsigned char *element);

extern void rustc_bls12_381_g2_add(unsigned char *buffer, const unsigned char* g1, const unsigned char *g2);

extern void rustc_bls12_381_g2_negate(unsigned char *buffer,
                                      const unsigned char *g);

extern bool rustc_bls12_381_g2_eq(const unsigned char *g1,
                                  const unsigned char *g2);

extern bool rustc_bls12_381_g2_is_zero(const unsigned char *g);

extern void rustc_bls12_381_g2_double(unsigned char *buffer,
                                      const unsigned char *g);

extern void rustc_bls12_381_g2_mul(unsigned char *buffer,
                                  const unsigned char *g,
                                  const unsigned char *a);

extern void
rustc_bls12_381_g2_compressed_of_uncompressed(unsigned char *buffer,
                                              const unsigned char *g);

extern void
rustc_bls12_381_g2_uncompressed_of_compressed(unsigned char *buffer,
                                              const unsigned char *g);

// ----------------- G2 Compressed ----------------------
extern void rustc_bls12_381_g2_compressed_one(unsigned char *buffer);

extern void rustc_bls12_381_g2_compressed_zero(unsigned char *buffer);

extern void rustc_bls12_381_g2_compressed_random(unsigned char *element);

extern void rustc_bls12_381_g2_compressed_add(unsigned char *buffer,
                                              const unsigned char* g1,
                                              const unsigned char *g2);

extern void rustc_bls12_381_g2_compressed_negate(unsigned char *buffer,
                                                 const unsigned char *g);

extern bool rustc_bls12_381_g2_compressed_eq(const unsigned char *g1,
                                             const unsigned char *g2);

extern bool rustc_bls12_381_g2_compressed_is_zero(const unsigned char *g);

extern void rustc_bls12_381_g2_compressed_double(unsigned char *buffer,
                                                 const unsigned char *g);

extern void rustc_bls12_381_g2_compressed_mul(unsigned char *buffer,
                                              const unsigned char *g,
                                              const unsigned char *a);

extern bool rustc_bls12_381_g2_build_from_components(unsigned char *buffer,
                                                     const unsigned char *x_1,
                                                     const unsigned char *x_2,
                                                     const unsigned char *y_1,
                                                     const unsigned char *y_2);

// Miller loop interfaces
extern void rustc_bls12_381_pairing_miller_loop_simple(unsigned char *buffer,
                                                       const unsigned char *g1,
                                                       const unsigned char *g2);

extern void rustc_bls12_381_pairing_miller_loop_2(unsigned char *buffer,
                                                  const unsigned char *g1_1,
                                                  const unsigned char *g1_2,
                                                  const unsigned char *g2_1,
                                                  const unsigned char *g2_2);

extern void rustc_bls12_381_pairing_miller_loop_3(
    unsigned char *buffer,
    const unsigned char *g1_1, const unsigned char *g1_2,
    const unsigned char *g1_3, const unsigned char *g2_1,
    const unsigned char *g2_2, const unsigned char *g2_3);

extern void rustc_bls12_381_pairing_miller_loop_4(
    unsigned char *buffer, const unsigned char *g1_1, const unsigned char *g1_2,
    const unsigned char *g1_3, const unsigned char *g1_4, const unsigned char *g2_1,
    const unsigned char *g2_2, const unsigned char *g2_3, const unsigned char *g2_4);

extern void rustc_bls12_381_pairing_miller_loop_5(
    unsigned char *buffer, const unsigned char *g1_1, const unsigned char *g1_2,
    const unsigned char *g1_3, const unsigned char *g1_4, const unsigned char *g1_5,
    const unsigned char *g2_1, const unsigned char *g2_2,
    const unsigned char *g2_3, const unsigned char *g2_4, const unsigned char *g2_5);

extern void rustc_bls12_381_pairing_miller_loop_6(
    unsigned char *buffer, const unsigned char *g1_1, const unsigned char *g1_2,
    const unsigned char *g1_3, const unsigned char *g1_4,
    const unsigned char *g1_5, const unsigned char *g1_6, const unsigned char *g2_1,
    const unsigned char *g2_2, const unsigned char *g2_3,
    const unsigned char *g2_4, const unsigned char *g2_5, const unsigned char *g2_6);

extern void rustc_bls12_381_pairing(unsigned char *buffer,
                                    const unsigned char *g1,
                                    const unsigned char *g2);

extern void rustc_bls12_381_unsafe_pairing_final_exponentiation(unsigned char *buffer,
                                                                const unsigned char *x);

#endif
