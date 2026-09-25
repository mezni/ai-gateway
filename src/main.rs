fn main() {
    let state = ai_gateway::application::AppState::new(ai_gateway::app_version());
    println!("{}", state.banner());
}
