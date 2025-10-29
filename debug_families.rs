use cllient::runtime::Runtime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new()?;
    
    println!("Available families:");
    let families = runtime.list_families();
    for family in &families {
        println!("  - {}", family);
        let models = runtime.list_models_in_family(family);
        println!("    Models: {:?}", models);
    }
    
    println!("\nLooking for claude models:");
    let claude_models = runtime.list_models_in_family("claude");
    println!("Claude family models: {:?}", claude_models);
    
    println!("\nAll models (first 10):");
    let all_models = runtime.list_models();
    for (i, model) in all_models.iter().take(10).enumerate() {
        if let Ok(info) = runtime.get_model_info(model) {
            println!("  {}: {} (family: {})", i+1, model, info.model.family);
        }
    }
    
    Ok(())
}