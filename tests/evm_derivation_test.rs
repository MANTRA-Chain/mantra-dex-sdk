#[cfg(all(test, feature = "evm"))]
mod tests {
    use mantra_sdk::wallet::MultiVMWallet;

    #[test]
    fn test_evm_address_derivation() {
        let mnemonic = "damage spring lunch thrive dumb shuffle enact metal force scissors black sound exit cabin park story eager quote town jacket thought host scorpion buffalo";

        println!("\nTesting EVM address derivation:");
        println!("================================");

        // Test first 5 derivation indices
        for index in 0..5 {
            let wallet = MultiVMWallet::from_mnemonic(mnemonic, index).unwrap();

            let cosmos_addr = wallet.cosmos_address().unwrap();
            let evm_addr = wallet.evm_address().unwrap();

            println!("\nIndex {index}:");
            println!("  Cosmos: {}", cosmos_addr);
            println!("  EVM:    {}", evm_addr);

            // Check if this matches the expected Metamask address
            if evm_addr.to_string() == "0x70c96781CDf0e7C1607cDd7162146F7447A12684" {
                println!("  ✅ MATCHES expected Metamask address!");
            }
        }
    }
}
