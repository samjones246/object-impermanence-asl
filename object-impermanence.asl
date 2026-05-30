state("Object Impermanence") {
    string255 activeCheckpointId: "GameAssembly.dll", 0x020A8978, 0xB8, 0x18, 0x20, 0x14;
    string255 scenePath: "GameAssembly.dll", 0x020A8978, 0xB8, 0x18, 0x18, 0x14;
    bool activeCheckpointIsMain: "GameAssembly.dll", 0x020A8978, 0xB8, 0x18, 0x28;
    byte respawnState: "GameAssembly.dll", 0x020AB398, 0xB8, 0x68, 0x38;
}

startup
{
    settings.Add("split_main_checkpoints", false, "Split on main checkpoints");   
    settings.Add("split_all_checkpoints", false, "Split on all checkpoints");   
    settings.Add("split_scene", true, "Split on area complete");
    settings.Add("split_scene_0_landing", true, "0 - Landing", "split_scene");
    settings.Add("split_scene_1a_intro", true, "1A - Intro", "split_scene");
    settings.Add("split_scene_1b_exterior", true, "1B - Exterior", "split_scene");
    settings.Add("split_scene_2a_spatial", true, "2A - Spatial", "split_scene");
}

start
{
    return current.activeCheckpointId == "2f4e2d6fd93506b48b9baef53f65c81c" && current.respawnState == 4 && old.respawnState == 3;
}

update
{
    current.sceneName = Path.GetFileNameWithoutExtension(current.scenePath).ToLower();
    if (current.sceneName != old.sceneName) {
        print("scene: " + current.sceneName);
    }
    if (current.activeCheckpointId != old.activeCheckpointId) {
        print("checkpoint: " + current.activeCheckpointId);
    }
    if (current.respawnState != old.respawnState) {
        print("respawn state: " + current.respawnState);
    }
}

split
{
	if (current.activeCheckpointId != old.activeCheckpointId) {
        if (current.activeCheckpointIsMain && settings["split_main_checkpoints"]) {
            return true;
        }
        if (settings["split_all_checkpoints"]) {
            return true;
        }
    }
    if (current.sceneName != old.sceneName && settings["split_scene_" + old.sceneName]) {
        return true;
    }
}

isLoading
{
    return current.respawnState != 4;
}