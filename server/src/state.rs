use core::constants::MAXPLAYER;
use std::rc::Rc;

use crate::enums;
use crate::network_manager::NetworkManager;
use crate::path_finding::PathFinder;
use crate::repository::Repository;

pub struct State {
    pathfinder: PathFinder,
    network: Rc<NetworkManager>,
}

impl State {
    pub fn new(network: Rc<NetworkManager>) -> Self {
        State {
            // Initialize fields as necessary
            pathfinder: PathFinder::new(),
            network,
        }
    }

    /// plr_logout from original C++ code
    pub fn logout_player(
        &mut self,
        repository: &mut Repository,
        character_id: usize,
        player: Option<(usize, &mut core::types::ServerPlayer)>,
        reason: enums::LogoutReason,
    ) {
        // Logic to log out the player
        log::debug!(
            "Logging out character '{}' for reason: {:?}",
            repository.characters[character_id].get_name(),
            reason
        );

        let character_has_player = repository.characters[character_id].player
            == player.map_or(i32::MAX, |(player_number, _)| player_number as i32);

        if character_has_player
            && repository.characters[character_id].flags & enums::CharacterFlags::Usurp.bits() != 0
        {
            // If this character belongs to the player being logged out and has the Usurp flag set,
            // clear all elevated privilege flags (CCP, Usurp, Staff, Immortal, God, Creator).
            // This ensures that characters who were temporarily granted elevated permissions
            // (e.g., via usurping another character) lose those permissions when logging out.
            repository.characters[character_id].flags &=
                !(enums::CharacterFlags::ComputerControlledPlayer
                    | enums::CharacterFlags::Usurp
                    | enums::CharacterFlags::Staff
                    | enums::CharacterFlags::Immortal
                    | enums::CharacterFlags::God
                    | enums::CharacterFlags::Creator)
                    .bits();

            let co = repository.characters[character_id].data[97];
            // Perform the actual logout operation for the player controlling this character.
            // TODO: Handle the player reference properly.
            self.logout_player(repository, co as usize, None, enums::LogoutReason::Shutdown);
        }

        // TODO: Evaluate if we can use this instead:
        // if character_has_player
        //     && enums::CharacterFlags::from_bits_truncate(repository.characters[character_id].flags)
        //         .contains(enums::CharacterFlags::Player)
        //     && !enums::CharacterFlags::from_bits_truncate(repository.characters[character_id].flags)
        //         .contains(enums::CharacterFlags::ComputerControlledPlayer)
        // {}
        if character_has_player
            && (repository.characters[character_id].flags & enums::CharacterFlags::Player.bits())
                != 0
            && (repository.characters[character_id].flags
                & enums::CharacterFlags::ComputerControlledPlayer.bits())
                == 0
        {
            if reason == enums::LogoutReason::Exit {
                log::warn!(
                    "Character '{}' punished for leaving the game by means of F12.",
                    repository.characters[character_id].get_name(),
                );
            }
        }
    }

    pub fn do_character_log(
        &mut self,
        repository: &mut Repository,
        character_id: usize,
        player: Option<(usize, &mut core::types::ServerPlayer)>,
        font: core::types::FontColor,
        message: &str,
    ) {
        if repository.characters[character_id].player == 0
            && repository.characters[character_id].temp != 15
        {
            return;
        }

        self.do_log(repository, character_id, player, font, message);
    }

    pub fn do_log(
        &mut self,
        repository: &mut Repository,
        character_id: usize,
        player: Option<(usize, &mut core::types::ServerPlayer)>,
        font: core::types::FontColor,
        message: &str,
    ) {
        let buffer: [u8; 16] = [0; 16];

        let player_id = repository.characters[character_id].player;

        if player_id < 1 || player_id as usize >= MAXPLAYER {
            log::error!(
                "do_log: Invalid player ID {} for character '{}'",
                player_id,
                repository.characters[character_id].get_name(),
            );
            return;
        }

        if let Some((_, player)) = player {
            if player.usnr != character_id {
                return;
            }
        } else {
            log::warn!(
                "do_log: No player reference for character '{}'",
                repository.characters[character_id].get_name(),
            );
        }

        self.network.send(player_id as usize, &buffer, 16)
    }
}
