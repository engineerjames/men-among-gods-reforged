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

        let mut character = &mut repository.characters[character_id];

        let character_has_player = character.player
            == player
                .as_ref()
                .map_or(i32::MAX, |(player_number, _)| *player_number as i32);

        if character_has_player && character.flags & enums::CharacterFlags::Usurp.bits() != 0 {
            // If this character belongs to the player being logged out and has the Usurp flag set,
            // clear all elevated privilege flags (CCP, Usurp, Staff, Immortal, God, Creator).
            // This ensures that characters who were temporarily granted elevated permissions
            // (e.g., via usurping another character) lose those permissions when logging out.
            character.flags &= !(enums::CharacterFlags::ComputerControlledPlayer
                | enums::CharacterFlags::Usurp
                | enums::CharacterFlags::Staff
                | enums::CharacterFlags::Immortal
                | enums::CharacterFlags::God
                | enums::CharacterFlags::Creator)
                .bits();

            let co = character.data[97];
            // Perform the actual logout operation for the player controlling this character.
            // TODO: Handle the player reference properly.
            drop(character);

            self.logout_player(repository, co as usize, None, enums::LogoutReason::Shutdown);

            character = &mut repository.characters[character_id];
        }

        // TODO: Evaluate if we can use this instead:
        // if character_has_player
        //     && enums::CharacterFlags::from_bits_truncate(repository.characters[character_id].flags)
        //         .contains(enums::CharacterFlags::Player)
        //     && !enums::CharacterFlags::from_bits_truncate(repository.characters[character_id].flags)
        //         .contains(enums::CharacterFlags::ComputerControlledPlayer)
        // {}
        if character_has_player
            && (character.flags & enums::CharacterFlags::Player.bits()) != 0
            && (character.flags & enums::CharacterFlags::ComputerControlledPlayer.bits()) == 0
        {
            if reason == enums::LogoutReason::Exit {
                log::warn!(
                    "Character '{}' punished for leaving the game by means of F12.",
                    character.get_name(),
                );

                let damage_message = format!(
                    "You have been hit by a demon. You lost {} HP.\n",
                    (character.hp[5] * 8 / 10)
                );
                let messages_to_send = [
                    " \n",
                    "You are being punished for leaving the game without entering a tavern:\n",
                    " \n",
                    damage_message.as_str(),
                ];

                for i in 0..messages_to_send.len() {
                    self.do_character_log(
                        repository,
                        character_id,
                        &player,
                        core::types::FontColor::Red,
                        messages_to_send[i],
                    );
                }

                character.a_hp -= (character.hp[5] * 800) as i32;

                if character.a_hp < 500 {
                    self.do_character_log(
                        repository,
                        character_id,
                        &player,
                        core::types::FontColor::Red,
                        String::from("The demon killed you.\n \n").as_str(),
                    );
                    // TODO: Kill the character here
                } else {
                    if character.gold / 10 > 0 {
                        let money_stolen_message = format!(
                            " \nA demon grabs your purse and removes {} gold, and {} silver.\n",
                            (character.gold / 10) / 100,
                            (character.gold / 10) % 100
                        );

                        self.do_character_log(
                            repository,
                            character_id,
                            &player,
                            core::types::FontColor::Red,
                            money_stolen_message.as_str(),
                        );

                        character.gold -= character.gold / 10;

                        if character.citem != 0 && character.citem & 0x80000000 == 0 {
                            self.do_character_log(
                                repository,
                                character_id,
                                &player,
                                core::types::FontColor::Red,
                                "The demon also takes the money in your hand!\n",
                            );

                            // Remove the item from the character
                            character.citem = 0;
                        }

                        // TODO: Do area log here
                        //    do_area_log( cn, 0, ch[ cn ].x, ch[ cn ].y, 2, "%s left the game without saying goodbye and was punished by the gods.\n",
                        //    ch[ cn ].name );
                    }
                }
            }
        }
    }

    pub fn do_character_log(
        &self,
        repository: &Repository,
        character_id: usize,
        player: &Option<(usize, &mut core::types::ServerPlayer)>,
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
        &self,
        repository: &Repository,
        character_id: usize,
        player: &Option<(usize, &mut core::types::ServerPlayer)>,
        font: core::types::FontColor,
        message: &str,
    ) {
        let mut buffer: [u8; 16] = [0; 16];

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

        let mut bytes_sent: usize = 0;
        let len = message.len() - 1;

        while bytes_sent <= len {
            buffer[0] = core::constants::SV_LOG + font as u8;

            for i in 0..15 {
                if bytes_sent + i > len {
                    buffer[i + 1] = 0;
                } else {
                    buffer[i + 1] = message.as_bytes()[bytes_sent + i];
                }
            }

            self.network.send(player_id as usize, &buffer, 16);

            bytes_sent += 15;
        }
    }
}
