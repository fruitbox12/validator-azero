use ink_env::{AccountId, Hash};
use ink_storage::traits::{PackedLayout, SpreadLayout};
use scale::{Decode, Encode};

#[derive(Debug, Encode, Decode, Clone, Copy, SpreadLayout, PackedLayout, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
)]
pub enum Role {
    /// Indicates a superuser.
    Admin(AccountId),
    /// Indicates account that can terminate a contract.
    Owner(AccountId),
    /// Indicates account that can initialize a contract from a given code hash.
    Initializer(Hash),
    /// Indicates account that can add liquidity to a DEX contract (call certain functions)
    LiquidityProvider(AccountId),
    /// Indicates account that can mint tokens of a given token contract,
    Minter(AccountId),
    /// Indicates account that can burn tokens of a given token contract,
    Burner(AccountId),
}
