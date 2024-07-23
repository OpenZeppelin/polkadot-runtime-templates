import "@moonbeam-network/api-augment";
import { beforeAll, describeSuite, expect } from "@moonwall/cli";
// import { MoonwallContext, beforeAll, describeSuite, expect } from "@moonwall/cli";
// import { BALTATHAR_ADDRESS, alith, charleth } from "@moonwall/util";
import { ApiPromise } from "@polkadot/api";
// import { ethers } from "ethers";
// import fs from "node:fs";

describeSuite({
  id: "Z01",
  title: "Zombie AlphaNet Upgrade Test",
  foundationMethods: "zombie",
  testCases: ({ it, context, log }) => {
    let paraApi: ApiPromise;
    // let relayApi: ApiPromise;

    beforeAll(async () => {
      paraApi = context.polkadotJs("parachain");
      // relayApi = context.polkadotJs("relaychain");

      const currentBlock = (await paraApi.rpc.chain.getBlock()).block.header.number.toNumber();
      expect(currentBlock, "Parachain not producing blocks").to.be.greaterThan(0);
    }, 120000);

    it({
      id: "T01",
      title: "Blocks are being produced on parachain",
      test: async () => {
        const blockNum = (await paraApi.rpc.chain.getBlock()).block.header.number.toNumber();
        expect(blockNum).to.be.greaterThan(0);
      },
    });

    // it({
    //   id: "T02",
    //   title: "Can connect to parachain and execute a transaction",
    //   timeout: 240000,
    //   test: async () => {
    //     const balBefore = (await paraApi.query.system.account(BALTATHAR_ADDRESS)).data.free;

    //     log("Please wait, this will take at least 30s for transaction to complete");

    //     // TODO: Renable the below when we are using polkadot 1.7.0
    //     //       There is a discrepancy with polkadotJs and 1.3.0
    //     //
    //     // await new Promise((resolve, reject) => {
    //     //   paraApi.tx.balances
    //     //     .transferAllowDeath(BALTATHAR_ADDRESS, ethers.parseEther("2"))
    //     //     .signAndSend(charleth, ({ status, events }) => {
    //     //       if (status.isInBlock) {
    //     //         log("Transaction is in block");
    //     //       }
    //     //       if (status.isFinalized) {
    //     //         log("Transaction is finalized!");
    //     //         resolve(events);
    //     //       }

    //     //       if (
    //     //         status.isDropped ||
    //     //         status.isInvalid ||
    //     //         status.isUsurped ||
    //     //         status.isFinalityTimeout
    //     //       ) {
    //     //         reject("transaction failed!");
    //     //         throw new Error("Transaction failed");
    //     //       }
    //     //     });
    //     // })

    //     await paraApi.tx.balances
    //       .transferAllowDeath(BALTATHAR_ADDRESS, ethers.parseEther("2"))
    //       .signAndSend(charleth);

    //     // TODO: Remove waitBlock below when we are using polkadot 1.7.0
    //     await context.waitBlock(6);

    //     const balAfter = (await paraApi.query.system.account(BALTATHAR_ADDRESS)).data.free;
    //     expect(
    //       balBefore.lt(balAfter),
    //       `${balBefore.toHuman()} is not less than ${balAfter.toHuman()}`
    //     ).to.be.true;
    //   },
    // });

    // it({
    //   id: "T03",
    //   title: "Tags are present on emulated Ethereum blocks",
    //   test: async () => {
    //     expect(
    //       (await context.ethers().provider?.getBlock("safe"))?.number,
    //       "Safe tag is not present"
    //     ).to.be.greaterThan(0);
    //     expect(
    //       (await context.ethers().provider?.getBlock("finalized"))?.number,
    //       "Finalized tag is not present"
    //     ).to.be.greaterThan(0);
    //     expect(
    //       (await context.ethers().provider?.getBlock("latest"))?.number,
    //       "Latest tag is not present"
    //     ).to.be.greaterThan(0);
    //     // log(await ethersSigner.provider.getTransactionCount(ALITH_ADDRESS, "latest"));
    //     // await context
    //     //   .ethers()
    //     //   .sendTransaction({ to: BALTATHAR_ADDRESS, value: ethers.parseEther("1") });
    //     // log(await ethersSigner.provider.getTransactionCount(ALITH_ADDRESS, "pending"));
    //   },
    // });
  },
});
