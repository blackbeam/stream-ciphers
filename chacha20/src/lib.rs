#![no_std]
extern crate block_cipher_trait;
extern crate salsa20_core;
extern crate stream_cipher;

use block_cipher_trait::generic_array::typenum::{U12, U32, U8};
use block_cipher_trait::generic_array::{ArrayLength, GenericArray};
use salsa20_core::{SalsaFamilyCipher, SalsaFamilyState};
use stream_cipher::{NewStreamCipher, StreamCipher, SyncStreamCipherSeek};

/// The ChaCha20 stream cipher (RFC 7539 version with 96-bit nonce)
///
/// Use `ChaCha20Legacy` for the legacy (a.k.a. "djb") construction with a
/// 64-bit nonce.
pub struct ChaCha20(ChaChaState<U12>);

/// The ChaCha20 stream cipher (legacy "djb" construction with 64-bit nonce).
///
/// The `legacy` Cargo feature must be enabled to use this.
#[cfg(feature = "legacy")]
pub struct ChaCha20Legacy(ChaChaState<U8>);

macro_rules! impl_chacha20 {
    ($type:path, $noncesize:ty) => {
        impl NewStreamCipher for $type {
            /// Key size in bytes
            type KeySize = U32;

            /// Nonce size in bytes
            type NonceSize = $noncesize;

            fn new(
                key: &GenericArray<u8, Self::KeySize>,
                iv: &GenericArray<u8, Self::NonceSize>,
            ) -> Self {
                let mut out = $type(ChaChaState::new(key, iv));
                out.0.gen_block();
                out
            }
        }

        impl core::ops::Deref for $type {
            type Target = ChaChaState<$noncesize>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl core::ops::DerefMut for $type {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}

impl_chacha20!(ChaCha20, U12);

#[cfg(feature = "legacy")]
impl_chacha20!(ChaCha20Legacy, U8);

/// Wrapper for state for ChaCha-type ciphers.
pub struct ChaChaState<N: ArrayLength<u8>> {
    state: SalsaFamilyState,
    phantom: core::marker::PhantomData<N>,
}

impl<N: ArrayLength<u8>> ChaChaState<N> {
    pub(crate) fn gen_block(&mut self) {
        self.init_block();
        self.rounds();
        self.add_block();
    }

    #[inline]
    fn rounds(&mut self) {
        self.double_round();
        self.double_round();
        self.double_round();
        self.double_round();
        self.double_round();

        self.double_round();
        self.double_round();
        self.double_round();
        self.double_round();
        self.double_round();
    }

    #[inline]
    fn double_round(&mut self) {
        let block = &mut self.state.block;

        quarter_round(0, 4, 8, 12, block);
        quarter_round(1, 5, 9, 13, block);
        quarter_round(2, 6, 10, 14, block);
        quarter_round(3, 7, 11, 15, block);
        quarter_round(0, 5, 10, 15, block);
        quarter_round(1, 6, 11, 12, block);
        quarter_round(2, 7, 8, 13, block);
        quarter_round(3, 4, 9, 14, block);
    }

    #[inline]
    fn add_block(&mut self) {
        let block = &mut self.state.block;
        let iv = self.state.iv;
        let key = self.state.key;
        let block_idx = self.state.block_idx;

        block[0] = block[0].wrapping_add(0x6170_7865);
        block[1] = block[1].wrapping_add(0x3320_646e);
        block[2] = block[2].wrapping_add(0x7962_2d32);
        block[3] = block[3].wrapping_add(0x6b20_6574);
        block[4] = block[4].wrapping_add(key[0]);
        block[5] = block[5].wrapping_add(key[1]);
        block[6] = block[6].wrapping_add(key[2]);
        block[7] = block[7].wrapping_add(key[3]);
        block[8] = block[8].wrapping_add(key[4]);
        block[9] = block[9].wrapping_add(key[5]);
        block[10] = block[10].wrapping_add(key[6]);
        block[11] = block[11].wrapping_add(key[7]);
        block[12] = block[12].wrapping_add((block_idx & 0xffff_ffff) as u32);
        block[13] = block[13].wrapping_add(((block_idx >> 32) & 0xffff_ffff) as u32);
        block[14] = block[14].wrapping_add(iv[0]);
        block[15] = block[15].wrapping_add(iv[1]);
    }

    #[inline]
    fn init_block(&mut self) {
        let block = &mut self.state.block;
        let iv = self.state.iv;
        let key = self.state.key;
        let block_idx = self.state.block_idx;

        block[0] = 0x6170_7865;
        block[1] = 0x3320_646e;
        block[2] = 0x7962_2d32;
        block[3] = 0x6b20_6574;
        block[4] = key[0];
        block[5] = key[1];
        block[6] = key[2];
        block[7] = key[3];
        block[8] = key[4];
        block[9] = key[5];
        block[10] = key[6];
        block[11] = key[7];
        block[12] = (block_idx & 0xffff_ffff) as u32;
        block[13] = ((block_idx >> 32) & 0xffff_ffff) as u32;
        block[14] = iv[0];
        block[15] = iv[1];
    }
}

impl NewStreamCipher for ChaChaState<U8> {
    /// Key size in bytes
    type KeySize = U32;

    /// Nonce size in bytes
    type NonceSize = U8;

    fn new(key: &GenericArray<u8, Self::KeySize>, iv: &GenericArray<u8, Self::NonceSize>) -> Self {
        ChaChaState {
            state: SalsaFamilyState::new(key, iv),
            phantom: core::marker::PhantomData,
        }
    }
}

impl NewStreamCipher for ChaChaState<U12> {
    /// Key size in bytes
    type KeySize = U32;

    /// Nonce size in bytes
    type NonceSize = U12;

    fn new(key: &GenericArray<u8, Self::KeySize>, iv: &GenericArray<u8, Self::NonceSize>) -> Self {
        let exp_iv = &iv[0..4];
        let base_iv = &iv[4..12];

        let mut ccs = ChaChaState {
            state: SalsaFamilyState::new(key, GenericArray::from_slice(base_iv)),
            phantom: core::marker::PhantomData,
        };

        ccs.state.block_idx = (u64::from(exp_iv[0]) & 0xff) << 32
            | (u64::from(exp_iv[1]) & 0xff) << 40
            | (u64::from(exp_iv[2]) & 0xff) << 48
            | (u64::from(exp_iv[3]) & 0xff) << 56;

        ccs
    }
}

impl<N: ArrayLength<u8>> SalsaFamilyCipher for ChaChaState<N> {
    #[inline]
    fn next_block(&mut self) {
        self.state.block_idx += 1;
        self.gen_block();
    }

    #[inline]
    fn offset(&self) -> usize {
        self.state.offset
    }

    #[inline]
    fn set_offset(&mut self, offset: usize) {
        self.state.offset = offset;
    }

    #[inline]
    fn block_word(&self, idx: usize) -> u32 {
        self.state.block[idx]
    }
}

impl<N: ArrayLength<u8>> SyncStreamCipherSeek for ChaChaState<N> {
    fn current_pos(&self) -> u64 {
        self.state.current_pos()
    }

    fn seek(&mut self, pos: u64) {
        self.state.seek(pos);
        self.gen_block();
    }
}

impl<N: ArrayLength<u8>> StreamCipher for ChaChaState<N> {
    fn encrypt(&mut self, data: &mut [u8]) {
        self.process(data);
    }

    fn decrypt(&mut self, data: &mut [u8]) {
        self.process(data);
    }
}

#[inline]
fn quarter_round(a: usize, b: usize, c: usize, d: usize, block: &mut [u32; 16]) {
    block[a] = block[a].wrapping_add(block[b]);
    block[d] ^= block[a];
    block[d] = block[d].rotate_left(16);

    block[c] = block[c].wrapping_add(block[d]);
    block[b] ^= block[c];
    block[b] = block[b].rotate_left(12);

    block[a] = block[a].wrapping_add(block[b]);
    block[d] ^= block[a];
    block[d] = block[d].rotate_left(8);

    block[c] = block[c].wrapping_add(block[d]);
    block[b] ^= block[c];
    block[b] = block[b].rotate_left(7);
}