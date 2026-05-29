use asr::game_engine::unity::il2cpp::Module;
use asr::{future::next_tick, Process};

asr::async_main!(stable);
// asr::panic_handler!();

async fn main() {
    // TODO: Set up some general state and settings.

    asr::print_message("Hello, World!");

    loop {
        let process = Process::wait_attach("Object Impermanence.exe").await;
        process
            .until_closes(async {
                let module = Module::wait_attach_auto_detect(&process).await;
                let image = module.wait_get_default_image(&process).await;
                asr::print_message("Got image!");
                let timer_class = image
                    .wait_get_class(&process, &module, "SpeedrunTimer")
                    .await;
                let instance = timer_class
                    .wait_get_static_instance(&process, &module, "Instance")
                    .await;
                let current_time_offset = timer_class
                    .wait_get_field_offset(&process, &module, "time_elapsed_full")
                    .await;

                loop {
                    // Now we can add it to the address of the instance and read the current time.
                    if let Ok(current_time) = process.read::<f64>(instance + current_time_offset) {
                        asr::timer::set_variable("current_time", &current_time.to_string());
                    }
                    next_tick().await;
                }
            })
            .await;
    }
}
