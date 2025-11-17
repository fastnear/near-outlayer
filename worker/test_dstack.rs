use dstack_sdk::dstack_client::DstackClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create dstack client (will try to connect to /var/run/dstack.sock)
    let client = DstackClient::new(None);
    
    // Create dummy report_data (64 bytes)
    let mut report_data = [0u8; 64];
    report_data[0] = 0xaa; // some test data
    
    println!("Calling dstack client.get_quote()...");
    
    match client.get_quote(report_data.to_vec()).await {
        Ok(response) => {
            println!("✅ Success!");
            println!("Quote (HEX, first 100 chars): {}", 
                if response.quote.len() > 100 { &response.quote[..100] } else { &response.quote });
            println!("Quote length: {} hex chars ({} bytes)", 
                response.quote.len(), response.quote.len() / 2);
                
            // Try to decode from HEX
            match hex::decode(&response.quote) {
                Ok(bytes) => println!("✅ Successfully decoded from HEX, {} bytes", bytes.len()),
                Err(e) => println!("❌ Failed to decode from HEX: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Failed: {}", e);
        }
    }
    
    Ok(())
}
