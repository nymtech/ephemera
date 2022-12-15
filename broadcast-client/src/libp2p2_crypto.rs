///! Copy from broadcast workspace

#[derive(Debug)]
pub enum KeyPairError {
    InvalidSliceLength,
    InvalidSignature,
}

pub trait KeyPair: Sized {
    type Seed: Default + AsRef<[u8]> + AsMut<[u8]> + Clone;
    type Signature: AsRef<[u8]>;

    fn verify<M: AsRef<[u8]>>(
        &self,
        message: M,
        signature: &Self::Signature,
    ) -> Result<(), KeyPairError>;

    fn sign<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError>;

    fn generate(seed: &Self::Seed) -> Result<Self, KeyPairError>;
}

#[derive(Debug)]
pub struct Libp2pKeypair(pub libp2p::identity::Keypair);

impl KeyPair for Libp2pKeypair {
    type Seed = [u8; 0];
    type Signature = String;

    fn verify<M: AsRef<[u8]>>(
        &self,
        message: M,
        sig_data: &Self::Signature,
    ) -> Result<(), KeyPairError> {
        if !self
            .0
            .public()
            .verify(message.as_ref(), sig_data.as_bytes())
        {
            return Err(KeyPairError::InvalidSignature);
        }
        Ok(())
    }

    fn sign<M: AsRef<[u8]>>(&self, message: M) -> Result<Self::Signature, KeyPairError> {
        let sig_data = self
            .0
            .sign(message.as_ref())
            .map_err(|_| KeyPairError::InvalidSignature)?;
        let hex = array_bytes::bytes2hex("", sig_data);
        Ok(hex)
    }

    fn generate(_: &Self::Seed) -> Result<Self, KeyPairError> {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        Ok(Libp2pKeypair(keypair))
    }
}
