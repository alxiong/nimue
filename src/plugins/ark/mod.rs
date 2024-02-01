//! This module contains utilities for working with Arkworks types
//! and aid in the Fiat-Shamir heuristic for protocols dealing with
//! field elements and group elements.
//!
//! # Examples
//!
//! Here's a protocol that does Fiat-Shamir without caring about the hash function used
//! or the serialization format.
//!
//! ```rust
//! use ark_ec::CurveGroup;
//! use ark_std::UniformRand;
//! use nimue::{IOPattern, Arthur, DuplexHash, ProofResult};
//! use nimue::plugins::ark::*;
//!
//! fn prove<G>(
//!     arthur: &mut Arthur,
//!     x: G::ScalarField,
//! ) -> ProofResult<&[u8]>
//! where
//!     G: CurveGroup,
//! {
//!     let k = G::ScalarField::rand(arthur.rng());
//!     arthur.add_points(&[G::generator() * k])?;
//!     let [c]: [G::ScalarField; 1] = arthur.challenge_scalars()?;
//!     arthur.add_scalars(&[k + c * x])?;
//!     Ok(arthur.transcript())
//! }
//! ```
//! The type constraint on [`crate::Arthur`] hints the compiler that we are going to be absorbing elements from the group `G` and squeezing challenges in the scalar field `G::ScalarField`. Similarly, we could have been squeezing out bytes.
//!
//! ```rust
//! # use ark_ec::CurveGroup;
//! # use ark_std::UniformRand;
//! # use ark_ff::PrimeField;
//! # use nimue::{IOPattern, Arthur, DuplexHash, ProofResult};
//! # use nimue::plugins::ark::*;
//!
//! fn prove<G>(
//!     arthur: &mut Arthur,
//!     x: G::ScalarField,
//! ) -> ProofResult<&[u8]>
//! where
//!     G: CurveGroup,
//!     Arthur: GroupWriter<G> + ByteChallenges,
//! {
//!     let k = G::ScalarField::rand(arthur.rng());
//!     arthur.add_points(&[G::generator() * k])?;
//!     let c_bytes = arthur.challenge_bytes::<16>()?;
//!     let c = G::ScalarField::from_le_bytes_mod_order(&c_bytes);
//!     arthur.add_scalars(&[k + c * x])?;
//!     Ok(arthur.transcript())
//! }
//! ```
/// Add public elements (field or group elements) to the protocol transcript.
mod common;
/// IO Pattern utilities.
mod iopattern;
/// (WIP) Support for the Poseidon Hash function.
pub mod poseidon;
/// Veririfer's utilities for decoding a transcript.
mod reader;
/// Prover's utilities for encoding into a transcript.
mod writer;

#[cfg(test)]
/// Tests for arkworks.
mod tests;

#[cfg(feature = "anemoi")]
pub mod anemoi;

pub use crate::traits::*;
pub use crate::{hash::Unit, Arthur, DuplexHash, IOPattern, Merlin, ProofError, ProofResult, Safe};

super::traits::field_traits!(ark_ff::Field);
super::traits::group_traits!(ark_ec::CurveGroup, Scalar: ark_ff::PrimeField);

/// Move a value from prime field F1 to prime field F2.
///
/// Return an error if the element considered mod |F1| is different, when seen as an integer, mod |F2|.
/// This in particular happens when element > |F2|.
pub fn swap_field<F1: ark_ff::PrimeField, F2: ark_ff::PrimeField>(a_f1: F1) -> ProofResult<F2> {
    use ark_ff::BigInteger;
    let a_f2 = F2::from_le_bytes_mod_order(&a_f1.into_bigint().to_bytes_le());
    let a_f1_control = F1::from_le_bytes_mod_order(&a_f2.into_bigint().to_bytes_le());
    (a_f1 == a_f1_control)
        .then(|| a_f2)
        .ok_or(ProofError::SerializationError)
}

// pub trait PairingReader<P: ark_ec::pairing::Pairing>: GroupReader<P::G1> + GroupReader<P::G2>  {
//     fn fill_next_g1_points(&mut self, input: &mut [P::G1]) -> crate::ProofResult<()> {
//         GroupReader::<P::G1>::fill_next_points(self, input)
//     }

//     fn fill_next_g2_points(&mut self, input: &mut [P::G2]) -> crate::ProofResult<()> {
//         GroupReader::<P::G2>::fill_next_points(self, input)
//     }
// }
// pub trait PairingWriter<P: ark_ec::pairing::Pairing> {
//     fn add_g1_points(&mut self, input: &[P::G1]) -> crate::ProofResult<()> {
//         GroupWriter::<P::G1>::add_points(self, input)
//     }

//     fn add_g2_points(&mut self, input: &[P::G2]) -> crate::ProofResult<()> {
//         GroupWriter::<P::G2>::add_points(self, input)
//     }
// }

// impl<'a, P: ark_ec::pairing::Pairing, H, U> PairingWriter<P> for Merlin<'a, H, U> where
// U: Unit, H: DuplexHash<U>,
// Merlin<'a, H, U>:  GroupWriter<P::G1> + GroupWriter<P::G2>  {}
