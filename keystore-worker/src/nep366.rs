//! NEP-366 / NEP-461 delegate-action wire helpers.
//!
//! This module intentionally matches the current sequential gate: one
//! FunctionCall action per delegate, Ed25519 keys only, and byte layout compatible with
//! `near-primitives::action::delegate`.

use borsh::{BorshDeserialize, BorshSerialize};
use ed25519_dalek::{Signer, SigningKey};
use near_primitives::types::AccountId;
use sha2::{Digest, Sha256};
use std::io::{Error, ErrorKind, Read, Write};

pub const MIN_ON_CHAIN_DISCRIMINANT: u32 = 1 << 30;
pub const NEP_366_DELEGATE_DISCRIMINANT: u32 = MIN_ON_CHAIN_DISCRIMINANT + 366;

const ACTION_TAG_FUNCTION_CALL: u8 = 2;
const ACTION_TAG_DELEGATE: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519PublicKey(pub [u8; 32]);

impl BorshSerialize for Ed25519PublicKey {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        0u8.serialize(writer)?;
        writer.write_all(&self.0)
    }
}

impl BorshDeserialize for Ed25519PublicKey {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let tag = u8::deserialize_reader(reader)?;
        if tag != 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("expected ed25519 public key (tag 0), got tag {}", tag),
            ));
        }

        let mut bytes = [0u8; 32];
        reader.read_exact(&mut bytes)?;
        Ok(Self(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519Signature(pub [u8; 64]);

impl BorshSerialize for Ed25519Signature {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        0u8.serialize(writer)?;
        writer.write_all(&self.0)
    }
}

impl BorshDeserialize for Ed25519Signature {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let tag = u8::deserialize_reader(reader)?;
        if tag != 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("expected ed25519 signature (tag 0), got tag {}", tag),
            ));
        }

        let mut bytes = [0u8; 64];
        reader.read_exact(&mut bytes)?;
        Ok(Self(bytes))
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct FunctionCallAction {
    pub method_name: String,
    pub args: Vec<u8>,
    pub gas: u64,
    pub deposit: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonDelegateAction {
    FunctionCall(FunctionCallAction),
}

impl BorshSerialize for NonDelegateAction {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Self::FunctionCall(function_call) => {
                ACTION_TAG_FUNCTION_CALL.serialize(writer)?;
                function_call.serialize(writer)
            }
        }
    }
}

impl BorshDeserialize for NonDelegateAction {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let tag = u8::deserialize_reader(reader)?;
        match tag {
            ACTION_TAG_FUNCTION_CALL => Ok(Self::FunctionCall(
                FunctionCallAction::deserialize_reader(reader)?,
            )),
            ACTION_TAG_DELEGATE => Err(Error::new(
                ErrorKind::InvalidInput,
                "nested DelegateAction forbidden (NEP-366)",
            )),
            other => Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "action variant {} not supported in sequential delegates (only FunctionCall)",
                    other
                ),
            )),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct DelegateAction {
    pub sender_id: AccountId,
    pub receiver_id: AccountId,
    pub actions: Vec<NonDelegateAction>,
    pub nonce: u64,
    pub max_block_height: u64,
    pub public_key: Ed25519PublicKey,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct SignedDelegateAction {
    pub delegate_action: DelegateAction,
    pub signature: Ed25519Signature,
}

impl DelegateAction {
    pub fn signed_message_hash(&self) -> std::io::Result<[u8; 32]> {
        let mut bytes = Vec::with_capacity(128);
        NEP_366_DELEGATE_DISCRIMINANT.serialize(&mut bytes)?;
        self.serialize(&mut bytes)?;

        let digest = Sha256::digest(&bytes);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&digest);
        Ok(hash)
    }

    pub fn require_single_function_call(&self) -> Result<&FunctionCallAction, &'static str> {
        if self.actions.len() != 1 {
            return Err("sequential delegates require exactly one FunctionCall action");
        }

        match &self.actions[0] {
            NonDelegateAction::FunctionCall(function_call) => Ok(function_call),
        }
    }
}

pub struct FunctionCallDelegateParams {
    pub sender_id: AccountId,
    pub receiver_id: AccountId,
    pub method_name: String,
    pub args: Vec<u8>,
    pub gas: u64,
    pub deposit: u128,
    pub nonce: u64,
    pub max_block_height: u64,
    pub public_key: [u8; 32],
}

pub fn sign_function_call_delegate(
    params: FunctionCallDelegateParams,
    signing_key: &SigningKey,
) -> anyhow::Result<(SignedDelegateAction, [u8; 32])> {
    let delegate_action = DelegateAction {
        sender_id: params.sender_id,
        receiver_id: params.receiver_id,
        actions: vec![NonDelegateAction::FunctionCall(FunctionCallAction {
            method_name: params.method_name,
            args: params.args,
            gas: params.gas,
            deposit: params.deposit,
        })],
        nonce: params.nonce,
        max_block_height: params.max_block_height,
        public_key: Ed25519PublicKey(params.public_key),
    };

    delegate_action
        .require_single_function_call()
        .map_err(|message| anyhow::anyhow!(message))?;

    let hash = delegate_action.signed_message_hash()?;
    let signature = signing_key.sign(&hash).to_bytes();

    Ok((
        SignedDelegateAction {
            delegate_action,
            signature: Ed25519Signature(signature),
        },
        hash,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::BorshDeserialize;
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn params() -> FunctionCallDelegateParams {
        let signing_key = signing_key();
        FunctionCallDelegateParams {
            sender_id: "1111111111111111111111111111111111111111111111111111111111111111"
                .parse()
                .unwrap(),
            receiver_id: "intents.near".parse().unwrap(),
            method_name: "execute_intents".to_string(),
            args: br#"{"signed":["payload"]}"#.to_vec(),
            gas: 30_000_000_000_000,
            deposit: 1,
            nonce: 42,
            max_block_height: 1_000_000,
            public_key: signing_key.verifying_key().to_bytes(),
        }
    }

    #[test]
    fn discriminant_borsh_bytes_are_nep461_delegate_prefix() {
        let mut bytes = Vec::new();
        NEP_366_DELEGATE_DISCRIMINANT.serialize(&mut bytes).unwrap();
        assert_eq!(NEP_366_DELEGATE_DISCRIMINANT, 1_073_742_190);
        assert_eq!(bytes, vec![0x6E, 0x01, 0x00, 0x40]);
    }

    #[test]
    fn signed_delegate_round_trips_and_verifies() {
        let signing_key = signing_key();
        let verifying_key = VerifyingKey::from(&signing_key);
        let (signed, hash) = sign_function_call_delegate(params(), &signing_key).unwrap();

        let signature = Signature::from_bytes(&signed.signature.0);
        verifying_key.verify(&hash, &signature).unwrap();

        let bytes = borsh::to_vec(&signed).unwrap();
        let parsed = SignedDelegateAction::try_from_slice(&bytes).unwrap();
        assert_eq!(parsed, signed);
    }

    #[test]
    fn delegate_wire_matches_near_primitives() {
        use near_primitives::action::delegate::{
            DelegateAction as NativeDelegateAction, NonDelegateAction as NativeNonDelegateAction,
            SignedDelegateAction as NativeSignedDelegateAction,
        };
        use near_primitives::action::{
            Action as NativeAction, FunctionCallAction as NativeFunctionCallAction,
        };

        let signing_key = signing_key();
        let (signed, hash) = sign_function_call_delegate(params(), &signing_key).unwrap();
        let delegate = &signed.delegate_action;
        let function_call = delegate.require_single_function_call().unwrap();

        let native_action = NativeNonDelegateAction::try_from(NativeAction::FunctionCall(
            Box::new(NativeFunctionCallAction {
                method_name: function_call.method_name.clone(),
                args: function_call.args.clone(),
                gas: function_call.gas,
                deposit: function_call.deposit,
            }),
        ))
        .unwrap();

        let native_delegate = NativeDelegateAction {
            sender_id: delegate.sender_id.clone(),
            receiver_id: delegate.receiver_id.clone(),
            actions: vec![native_action],
            nonce: delegate.nonce,
            max_block_height: delegate.max_block_height,
            public_key: format!(
                "ed25519:{}",
                bs58::encode(delegate.public_key.0).into_string()
            )
            .parse()
            .unwrap(),
        };

        assert_eq!(
            borsh::to_vec(delegate).unwrap(),
            borsh::to_vec(&native_delegate).unwrap()
        );
        assert_eq!(hash.as_slice(), native_delegate.get_nep461_hash().as_ref());

        let native_signed = NativeSignedDelegateAction {
            delegate_action: native_delegate,
            signature: format!("ed25519:{}", bs58::encode(signed.signature.0).into_string())
                .parse()
                .unwrap(),
        };

        assert_eq!(
            borsh::to_vec(&signed).unwrap(),
            borsh::to_vec(&native_signed).unwrap()
        );
        assert!(native_signed.verify());
    }

    #[test]
    fn signed_delegate_rejects_tampered_hash() {
        let signing_key = signing_key();
        let verifying_key = VerifyingKey::from(&signing_key);
        let (signed, mut hash) = sign_function_call_delegate(params(), &signing_key).unwrap();

        hash[0] ^= 1;
        let signature = Signature::from_bytes(&signed.signature.0);
        assert!(verifying_key.verify(&hash, &signature).is_err());
    }

    #[test]
    fn nested_delegate_variant_is_rejected() {
        let mut bytes = Vec::new();
        (1u32).serialize(&mut bytes).unwrap();
        ACTION_TAG_DELEGATE.serialize(&mut bytes).unwrap();

        let err = Vec::<NonDelegateAction>::try_from_slice(&bytes).unwrap_err();
        assert!(err.to_string().contains("nested DelegateAction forbidden"));
    }

    #[test]
    fn multi_action_delegate_is_not_valid_for_sequential_gate() {
        let signing_key = signing_key();
        let mut params = params();
        params.public_key = signing_key.verifying_key().to_bytes();
        let (mut signed, _) = sign_function_call_delegate(params, &signing_key).unwrap();
        let extra = match signed.delegate_action.actions[0].clone() {
            NonDelegateAction::FunctionCall(function_call) => {
                NonDelegateAction::FunctionCall(function_call)
            }
        };
        signed.delegate_action.actions.push(extra);

        assert_eq!(
            signed.delegate_action.require_single_function_call(),
            Err("sequential delegates require exactly one FunctionCall action")
        );
    }
}
