/*!
   Constructs for running test cases with more than two chains,
   together with the relayer setup with chain handles and foreign clients,
   as well as connected IBC connections with completed handshakes.
*/

use ibc_relayer::chain::handle::ChainHandle;

use crate::bootstrap::nary::connection::bootstrap_connections;
use crate::error::Error;
use crate::framework::base::{HasOverrides, TestConfigOverride};
use crate::framework::binary::chain::RelayerConfigOverride;
use crate::framework::binary::connection::{BinaryConnectionTest, ConnectionDelayOverride};
use crate::framework::binary::node::{NodeConfigOverride, NodeGenesisOverride};
use crate::relayer::driver::RelayerDriver;
use crate::types::config::TestConfig;
use crate::types::nary::chains::NaryConnectedChains;
use crate::types::nary::connection::ConnectedConnections;

use super::chain::{run_nary_chain_test, NaryChainTest};

pub fn run_nary_connection_test<Test, Overrides, const SIZE: usize>(
    test: &Test,
) -> Result<(), Error>
where
    Test: NaryConnectionTest<SIZE>,
    Test: HasOverrides<Overrides = Overrides>,
    Overrides: TestConfigOverride
        + NodeConfigOverride
        + NodeGenesisOverride
        + RelayerConfigOverride
        + ConnectionDelayOverride,
{
    run_nary_chain_test(&RunNaryConnectionTest::new(test))
}

/**
   This trait is implemented for test cases that need to have more than
   two chains running with connected connections.

  Test writers can use this to implement test cases that only
  need the connections without channel handshake.
*/
pub trait NaryConnectionTest<const SIZE: usize> {
    /// Test runner
    fn run<Handle: ChainHandle>(
        &self,
        config: &TestConfig,
        relayer: RelayerDriver,
        chains: NaryConnectedChains<Handle, SIZE>,
        connections: ConnectedConnections<Handle, SIZE>,
    ) -> Result<(), Error>;
}

/**
   A wrapper type that lifts a test case that implements [`NaryConnectionTest`]
   into a test case the implements [`NaryChainTest`].
*/
pub struct RunNaryConnectionTest<'a, Test, const SIZE: usize> {
    /// Inner test
    pub test: &'a Test,
}

pub struct RunBinaryAsNaryConnectionTest<'a, Test> {
    /// Inner test
    pub test: &'a Test,
}

impl<'a, Test, const SIZE: usize> RunNaryConnectionTest<'a, Test, SIZE>
where
    Test: NaryConnectionTest<SIZE>,
{
    pub fn new(test: &'a Test) -> Self {
        Self { test }
    }
}

impl<'a, Test> RunBinaryAsNaryConnectionTest<'a, Test>
where
    Test: BinaryConnectionTest,
{
    pub fn new(test: &'a Test) -> Self {
        Self { test }
    }
}

impl<'a, Test, Overrides, const SIZE: usize> NaryChainTest<SIZE>
    for RunNaryConnectionTest<'a, Test, SIZE>
where
    Test: NaryConnectionTest<SIZE>,
    Test: HasOverrides<Overrides = Overrides>,
    Overrides: ConnectionDelayOverride,
{
    fn run<Handle: ChainHandle>(
        &self,
        config: &TestConfig,
        relayer: RelayerDriver,
        chains: NaryConnectedChains<Handle, SIZE>,
    ) -> Result<(), Error> {
        let connection_delay = self.get_overrides().connection_delay();

        let connections = bootstrap_connections(
            chains.foreign_clients().clone(),
            connection_delay,
            config.bootstrap_with_random_ids,
        )?;

        self.test.run(config, relayer, chains, connections)?;

        Ok(())
    }
}

impl<'a, Test> NaryConnectionTest<2> for RunBinaryAsNaryConnectionTest<'a, Test>
where
    Test: BinaryConnectionTest,
{
    fn run<Handle: ChainHandle>(
        &self,
        config: &TestConfig,
        relayer: RelayerDriver,
        chains: NaryConnectedChains<Handle, 2>,
        connections: ConnectedConnections<Handle, 2>,
    ) -> Result<(), Error> {
        self.test
            .run(config, relayer, chains.into(), connections.into())
    }
}

impl<'a, Test, Overrides, const SIZE: usize> HasOverrides for RunNaryConnectionTest<'a, Test, SIZE>
where
    Test: HasOverrides<Overrides = Overrides>,
{
    type Overrides = Overrides;

    fn get_overrides(&self) -> &Self::Overrides {
        self.test.get_overrides()
    }
}

impl<'a, Test, Overrides> HasOverrides for RunBinaryAsNaryConnectionTest<'a, Test>
where
    Test: HasOverrides<Overrides = Overrides>,
{
    type Overrides = Overrides;

    fn get_overrides(&self) -> &Self::Overrides {
        self.test.get_overrides()
    }
}
