use std::sync::Arc;
use std::{
    fmt::{Display, Error as FmtError, Formatter},
    marker::PhantomData,
};

use aleph_primitives::{BlockNumber, ALEPH_ENGINE_ID};
use log::warn;
use sp_blockchain::{Backend, Error as ClientError};

use sp_runtime::traits::{Block as BlockT, Header as SubstrateHeader};

use crate::{
    justification::backwards_compatible_decode,
    sync::{
        substrate::{BlockId, Justification},
        BlockStatus, ChainStatus, Header, LOG_TARGET,
    },
    AlephJustification,
};

/// What can go wrong when checking chain status
#[derive(Debug)]
pub enum Error<B: BlockT> {
    MissingHash(B::Hash),
    MissingJustification(B::Hash),
    Client(ClientError),
    MismatchedId,
}

impl<B: BlockT> Display for Error<B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            MissingHash(hash) => {
                write!(
                    f,
                    "data availability problem: no block for existing hash {:?}",
                    hash
                )
            }
            MissingJustification(hash) => {
                write!(
                    f,
                    "data availability problem: no justification for finalized block with hash {:?}",
                    hash
                )
            }
            Client(e) => {
                write!(f, "substrate client error {}", e)
            }
            MismatchedId => write!(f, "the block number did not match the block hash"),
        }
    }
}

impl<B: BlockT> From<ClientError> for Error<B> {
    fn from(value: ClientError) -> Self {
        Error::Client(value)
    }
}

/// Substrate implementation of ChainStatus trait
#[derive(Clone)]
pub struct SubstrateChainStatus<B, BE>
where
    BE: Backend<B>,
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    client: BE,
    _phantom: PhantomData<B>,
}

impl<B, BE> SubstrateChainStatus<B, BE>
where
    BE: Backend<B>,
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    pub fn new(client: BE) -> Self {
        Self { client, _phantom: PhantomData }
    }

    fn hash_for_number(&self, number: BlockNumber) -> Result<Option<B::Hash>, ClientError> {
        self.client.hash(number)
    }

    fn header_for_hash(&self, hash: B::Hash) -> Result<Option<B::Header>, ClientError> {
        self.client.header(hash)
    }

    fn header(
        &self,
        id: &<B::Header as Header>::Identifier,
    ) -> Result<Option<B::Header>, Error<B>> {
        let maybe_header = self.header_for_hash(id.hash)?;
        match maybe_header
            .as_ref()
            .map(|header| header.number() == &id.number)
        {
            Some(false) => Err(Error::MismatchedId),
            _ => Ok(maybe_header),
        }
    }

    fn justification(&self, hash: B::Hash) -> Result<Option<AlephJustification>, ClientError> {
        let justification = match self
            .client
            .justifications(hash)?
            .and_then(|j| j.into_justification(ALEPH_ENGINE_ID))
        {
            Some(justification) => justification,
            None => return Ok(None),
        };

        match backwards_compatible_decode(justification) {
            Ok(justification) => Ok(Some(justification)),
            // This should not happen, as we only import correctly encoded justification.
            Err(e) => {
                warn!(
                    target: LOG_TARGET,
                    "Could not decode stored justification for block {:?}: {}", hash, e
                );
                Ok(None)
            }
        }
    }

    fn best_hash(&self) -> B::Hash {
        self.client.info().best_hash
    }

    fn finalized_hash(&self) -> B::Hash {
        self.client.info().finalized_hash
    }
}

impl<B, BE> ChainStatus<Justification<B::Header>> for SubstrateChainStatus<B, BE>
where
    BE: Backend<B>,
    B: BlockT,
    B::Header: SubstrateHeader<Number = BlockNumber>,
{
    type Error = Error<B>;

    fn finalized_at(
        &self,
        number: BlockNumber,
    ) -> Result<Option<Justification<B::Header>>, Self::Error> {
        let id = match self.hash_for_number(number)? {
            Some(hash) => BlockId { hash, number },
            None => return Ok(None),
        };
        match self.status_of(id)? {
            BlockStatus::Justified(justification) => Ok(Some(justification)),
            _ => Ok(None),
        }
    }

    fn status_of(
        &self,
        id: <B::Header as Header>::Identifier,
    ) -> Result<BlockStatus<Justification<B::Header>>, Self::Error> {
        let header = match self.header(&id)? {
            Some(header) => header,
            None => return Ok(BlockStatus::Unknown),
        };

        if let Some(raw_justification) = self.justification(id.hash)? {
            Ok(BlockStatus::Justified(Justification {
                header,
                raw_justification,
            }))
        } else {
            Ok(BlockStatus::Present(header))
        }
    }

    fn best_block(&self) -> Result<B::Header, Self::Error> {
        let best_hash = self.best_hash();

        self.header_for_hash(best_hash)?
            .ok_or(Error::MissingHash(best_hash))
    }

    fn top_finalized(&self) -> Result<Justification<B::Header>, Self::Error> {
        let finalized_hash = self.finalized_hash();

        let header = self
            .header_for_hash(finalized_hash)?
            .ok_or(Error::MissingHash(finalized_hash))?;
        let raw_justification = self
            .justification(finalized_hash)?
            .ok_or(Error::MissingJustification(finalized_hash))?;

        Ok(Justification {
            header,
            raw_justification,
        })
    }

    fn children(
        &self,
        id: <B::Header as Header>::Identifier,
    ) -> Result<Vec<B::Header>, Self::Error> {
        // This checks whether we have the block at all and the provided id is consistent.
        self.header(&id)?;
        Ok(self
            .client
            .children(id.hash)?
            .into_iter()
            .map(|hash| self.header_for_hash(hash))
            .collect::<Result<Vec<Option<B::Header>>, ClientError>>()?
            .into_iter()
            .flatten()
            .collect())
    }
}
