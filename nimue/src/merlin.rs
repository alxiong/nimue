use rand::{CryptoRng, RngCore};

use crate::hash::Unit;
use crate::{ByteWriter, IOPattern, Safe, UnitTranscript};

use super::hash::{DuplexHash, Keccak};
use super::{DefaultHash, DefaultRng, IOPatternError};

/// A cryptographically-secure random number generator that is bound to the protocol transcript.
///
/// For most public-coin protocols it is *vital* not to have two different verifier messages for the same prover message.
/// For this reason, we construct a Rng that will absorb whatever the verifier absorbs, and that in addition
/// it is seeded by a cryptographic random number generator (by default, [`rand::rngs::OsRng`]).
///
/// Every time the prover's sponge is squeeze, the state of the sponge is ratcheted, so that it can't be inverted and the randomness recovered.
#[derive(Clone)]
pub(crate) struct ProverRng<R: RngCore + CryptoRng> {
    /// The sponge that is used to generate the random coins.
    pub(crate) sponge: Keccak,
    /// The cryptographic random number generator that seeds the sponge.
    pub(crate) csrng: R,
}

impl<R: RngCore + CryptoRng> RngCore for ProverRng<R> {
    fn next_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        self.fill_bytes(buf.as_mut());
        u32::from_le_bytes(buf)
    }

    fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.fill_bytes(buf.as_mut());
        u64::from_le_bytes(buf)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // Seed (at most) 32 bytes of randomness from the CSRNG
        let len = usize::min(dest.len(), 32);
        self.csrng.fill_bytes(&mut dest[..len]);
        self.sponge.absorb_unchecked(&dest[..len]);
        // fill `dest` with the output of the sponge
        self.sponge.squeeze_unchecked(dest);
        // erase the state from the sponge so that it can't be reverted
        self.sponge.ratchet_unchecked();
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.sponge.squeeze_unchecked(dest);
        Ok(())
    }
}

impl<H, U, R> Merlin<H, U, R>
where
    H: DuplexHash<U>,
    R: RngCore + CryptoRng,
    U: Unit,
{
    pub fn new(io_pattern: &IOPattern<H, U>, csrng: R) -> Self {
        let safe = Safe::new(io_pattern);

        let mut sponge = Keccak::default();
        sponge.absorb_unchecked(io_pattern.as_bytes());
        let rng = ProverRng { sponge, csrng };

        Self {
            rng,
            safe,
            transcript: Vec::new(),
        }
    }
}

impl<U, H> From<&IOPattern<H, U>> for Merlin<H, U, DefaultRng>
where
    U: Unit,
    H: DuplexHash<U>,
{
    fn from(io_pattern: &IOPattern<H, U>) -> Self {
        Merlin::new(io_pattern, DefaultRng::default())
    }
}

/// [`Merlin`] is the prover state in an interactive proof system.
/// It internally holds the secret coins of the prover for zero-knowledge, and
/// has the hash function state for the verifier state.
///
/// Unless otherwise specified,
/// [`Merlin`] is set to work over bytes with [`DefaultHash`] and
/// rely on the default random number generator [`DefaultRng`].
#[derive(Clone)]
pub struct Merlin<H = DefaultHash, U = u8, R = DefaultRng>
where
    U: Unit,
    H: DuplexHash<U>,
    R: RngCore + CryptoRng,
{
    /// The randomness state of the prover.
    pub(crate) rng: ProverRng<R>,
    /// The public coins for the protocol
    pub(crate) safe: Safe<H, U>,
    /// The encoded data.
    pub(crate) transcript: Vec<u8>,
}

impl<H, U, R> Merlin<H, U, R>
where
    U: Unit,
    H: DuplexHash<U>,
    R: RngCore + CryptoRng,
{
    /// Add a slice `[U]` to the protocol transcript.
    /// The messages are also internally encoded in the protocol transcript,
    /// and used to re-seed the prover's random number generator.
    ///
    /// ```
    /// use nimue::{IOPattern, DefaultHash, ByteWriter};
    ///
    /// let io = IOPattern::<DefaultHash>::new("📝").absorb(20, "how not to make pasta 🤌");
    /// let mut merlin = io.to_merlin();
    /// assert!(merlin.add_bytes(&[0u8; 20]).is_ok());
    /// let result = merlin.add_bytes(b"1tbsp every 10 liters");
    /// assert!(result.is_err())
    /// ```
    #[inline(always)]
    pub fn add_units(&mut self, input: &[U]) -> Result<(), IOPatternError> {
        // let serialized = bincode::serialize(input).unwrap();
        // self.merlin.sponge.absorb_unchecked(&serialized);
        let old_len = self.transcript.len();
        self.safe.absorb(input)?;
        // write never fails on Vec<u8>
        U::write(input, &mut self.transcript).unwrap();
        self.rng
            .sponge
            .absorb_unchecked(&self.transcript[old_len..]);

        Ok(())
    }

    /// Ratchet the verifier's state.
    #[inline(always)]
    pub fn ratchet(&mut self) -> Result<(), IOPatternError> {
        self.safe.ratchet()
    }

    /// Return a reference to the random number generator associated to the protocol transcript.
    ///
    /// ```
    /// # use nimue::*;
    /// # use rand::RngCore;
    ///
    /// // The IO Pattern does not need to specify the private coins.
    /// let io = IOPattern::<DefaultHash>::new("📝");
    /// let mut merlin = io.to_merlin();
    /// assert_ne!(merlin.rng().next_u32(), 0, "You won the lottery!");
    /// let mut challenges = [0u8; 32];
    /// merlin.rng().fill_bytes(&mut challenges);
    /// assert_ne!(challenges, [0u8; 32]);
    /// ```
    #[inline(always)]
    pub fn rng(&mut self) -> &mut (impl CryptoRng + RngCore) {
        &mut self.rng
    }

    /// Return the current protocol transcript.
    /// The protocol transcript does not hold eny information about the length or the type of the messages being read.
    /// This is because the information is considered pre-shared within the [`IOPattern`].
    /// Additionally, since the verifier challenges are deterministically generated from the prover's messages,
    /// the transcript does not hold any of the verifier's messages.
    ///
    /// ```
    /// # use nimue::*;
    ///
    /// let io = IOPattern::<DefaultHash>::new("📝").absorb(8, "how to make pasta 🤌");
    /// let mut merlin = io.to_merlin();
    /// merlin.add_bytes(b"1tbsp:3l").unwrap();
    /// assert_eq!(merlin.transcript(), b"1tbsp:3l");
    /// ```
    pub fn transcript(&self) -> &[u8] {
        self.transcript.as_slice()
    }
}

impl<H, U, R> UnitTranscript<U> for Merlin<H, U, R>
where
    U: Unit,
    H: DuplexHash<U>,
    R: RngCore + CryptoRng,
{
    /// Add public messages to the protocol transcript.
    /// Messages input to this function are not added to the protocol transcript.
    /// They are however absorbed into the verifier's sponge for Fiat-Shamir, and used to re-seed the prover state.
    ///
    /// ```
    /// # use nimue::*;
    ///
    /// let io = IOPattern::<DefaultHash>::new("📝").absorb(20, "how not to make pasta 🙉");
    /// let mut merlin = io.to_merlin();
    /// assert!(merlin.public_bytes(&[0u8; 20]).is_ok());
    /// assert_eq!(merlin.transcript(), b"");
    /// ```
    fn public_units(&mut self, input: &[U]) -> Result<(), IOPatternError> {
        let len = self.transcript.len();
        self.add_units(input)?;
        self.transcript.truncate(len);
        Ok(())
    }

    /// Fill a slice with uniformly-distributed challenges from the verifier.
    fn fill_challenge_units(&mut self, output: &mut [U]) -> Result<(), IOPatternError> {
        self.safe.squeeze(output)
    }
}

impl<R: RngCore + CryptoRng> CryptoRng for ProverRng<R> {}

impl<H, U, R> core::fmt::Debug for Merlin<H, U, R>
where
    U: Unit,
    H: DuplexHash<U>,
    R: RngCore + CryptoRng,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.safe.fmt(f)
    }
}

impl<H, R> ByteWriter for Merlin<H, u8, R>
where
    H: DuplexHash<u8>,
    R: RngCore + CryptoRng,
{
    #[inline(always)]
    fn add_bytes(&mut self, input: &[u8]) -> Result<(), IOPatternError> {
        self.add_units(input)
    }
}
