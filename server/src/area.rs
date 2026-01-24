use core::area::AREAS;

use crate::repository::Repository;

pub fn is_in_pentagram_quest(cn: usize) -> bool {
    if !(1..crate::core::constants::MAXCHARS).contains(&cn) {
        return false;
    }

    let coords = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
    let x = coords.0;
    let y = coords.1;

    let n: [usize; 5] = [67, 68, 110, 113, 123];
    for i in 0..5 {
        let idx = n[i];
        if AREAS.get(idx).map_or(false, |a| a.contains(x, y)) {
            return true;
        }
    }
    false
}

// Unused in original implementation as well
#[allow(dead_code)]
pub fn get_area(cn: usize, verbose: bool) -> String {
    if !(1..crate::core::constants::MAXCHARS).contains(&cn) {
        return String::new();
    }

    let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
    let mut buf = String::new();
    let mut first = true;

    for a in AREAS.iter() {
        if a.contains(x, y) {
            if verbose {
                if first {
                    buf.push_str("In ");
                    first = false;
                } else {
                    buf.push_str(", in ");
                }
                match a.flag {
                    1 => buf.push_str("the "),
                    2 => buf.push_str("on "),
                    3 => buf.push_str("at "),
                    _ => {}
                }
                buf.push_str(a.name);
            } else {
                if !first {
                    buf.push_str(", ");
                }
                first = false;
                buf.push_str(a.name);
            }
        }
    }

    buf
}

pub fn get_area_m(x: i32, y: i32, verbose: bool) -> String {
    let mut buf = String::new();
    let mut first = true;

    for a in AREAS.iter() {
        if a.contains(x, y) {
            if verbose {
                if first {
                    buf.push_str("in ");
                    first = false;
                } else {
                    buf.push_str(", in ");
                }
                match a.flag {
                    1 => buf.push_str("the "),
                    2 => buf.push_str("on "),
                    3 => buf.push_str("at "),
                    _ => {}
                }
                buf.push_str(a.name);
            } else {
                if !first {
                    buf.push_str(", ");
                }
                first = false;
                buf.push_str(a.name);
            }
        }
    }

    buf
}

#[cfg(test)]
mod tests {
    use core::area::Area;

    use super::*;

    #[test]
    fn test_area_contains_basic() {
        let area = Area {
            x1: 10,
            y1: 20,
            x2: 30,
            y2: 40,
            name: "Test Area",
            flag: 0,
        };

        // Test points inside the area
        assert!(area.contains(10, 20)); // Bottom-left corner
        assert!(area.contains(30, 40)); // Top-right corner
        assert!(area.contains(20, 30)); // Center
        assert!(area.contains(15, 25)); // Inside

        // Test points outside the area
        assert!(!area.contains(9, 20)); // Left of area
        assert!(!area.contains(31, 30)); // Right of area
        assert!(!area.contains(20, 19)); // Below area
        assert!(!area.contains(20, 41)); // Above area
        assert!(!area.contains(5, 5)); // Far outside
    }

    #[test]
    fn test_area_contains_edge_cases() {
        let area = Area {
            x1: 0,
            y1: 0,
            x2: 0,
            y2: 0,
            name: "Point Area",
            flag: 0,
        };

        // Single point area
        assert!(area.contains(0, 0));
        assert!(!area.contains(1, 0));
        assert!(!area.contains(0, 1));
        assert!(!area.contains(-1, 0));
        assert!(!area.contains(0, -1));
    }

    #[test]
    fn test_area_contains_negative_coordinates() {
        let area = Area {
            x1: -10,
            y1: -20,
            x2: 10,
            y2: 20,
            name: "Centered Area",
            flag: 0,
        };

        // Test with negative coordinates
        assert!(area.contains(-5, -10));
        assert!(area.contains(0, 0));
        assert!(area.contains(5, 10));
        assert!(!area.contains(-15, 0));
        assert!(!area.contains(15, 0));
    }

    #[test]
    fn test_get_area_m_no_areas() {
        // Test coordinates that don't match any predefined areas
        let result = get_area_m(-1000, -1000, false);
        assert_eq!(result, "");

        let result_verbose = get_area_m(-1000, -1000, true);
        assert_eq!(result_verbose, "");
    }

    #[test]
    fn test_get_area_m_aston() {
        // Test coordinates in Aston (first area in AREAS)
        let result = get_area_m(500, 450, false);
        assert!(result.contains("Aston"));

        let result_verbose = get_area_m(500, 450, true);
        assert!(result_verbose.starts_with("in "));
        assert!(result_verbose.contains("Aston"));
    }

    #[test]
    fn test_get_area_m_verbose_formatting() {
        // Test different flag types for verbose output

        // Find an area with flag 1 (should get "the")
        for area in AREAS.iter() {
            if area.flag == 1 {
                let x = (area.x1 + area.x2) / 2;
                let y = (area.y1 + area.y2) / 2;
                let result = get_area_m(x, y, true);
                if result.contains(area.name) {
                    assert!(result.contains("in the "));
                    break;
                }
            }
        }

        // Find an area with flag 2 (should get "on")
        for area in AREAS.iter() {
            if area.flag == 2 {
                let x = (area.x1 + area.x2) / 2;
                let y = (area.y1 + area.y2) / 2;
                let result = get_area_m(x, y, true);
                if result.contains(area.name) {
                    assert!(result.contains("in on "));
                    break;
                }
            }
        }

        // Find an area with flag 3 (should get "at")
        for area in AREAS.iter() {
            if area.flag == 3 {
                let x = (area.x1 + area.x2) / 2;
                let y = (area.y1 + area.y2) / 2;
                let result = get_area_m(x, y, true);
                if result.contains(area.name) {
                    assert!(result.contains("in at "));
                    break;
                }
            }
        }
    }

    #[test]
    fn test_get_area_m_multiple_areas() {
        // Find coordinates that might be in multiple overlapping areas
        // This tests the comma separation logic

        // Test non-verbose mode with potential overlaps
        let result = get_area_m(500, 450, false);
        if result.contains(",") {
            // If there are multiple areas, they should be comma-separated
            let parts: Vec<&str> = result.split(", ").collect();
            assert!(parts.len() > 1);
        }

        // Test verbose mode with potential overlaps
        let result_verbose = get_area_m(500, 450, true);
        if result_verbose.contains(", in ") {
            // Multiple areas in verbose mode should have ", in " separator
            assert!(result_verbose.starts_with("in "));
        }
    }

    #[test]
    fn test_areas_data_integrity() {
        // Test that all areas have valid coordinates
        for (i, area) in AREAS.iter().enumerate() {
            assert!(
                area.x1 <= area.x2,
                "Area {} '{}' has invalid x coordinates: {} > {}",
                i,
                area.name,
                area.x1,
                area.x2
            );
            assert!(
                area.y1 <= area.y2,
                "Area {} '{}' has invalid y coordinates: {} > {}",
                i,
                area.name,
                area.y1,
                area.y2
            );
            assert!(!area.name.is_empty(), "Area {} has empty name", i);
        }
    }

    #[test]
    fn test_areas_flag_values() {
        // Test that all area flags are within expected range
        let valid_flags = [0, 1, 2, 3];
        for area in AREAS.iter() {
            assert!(
                valid_flags.contains(&area.flag),
                "Area '{}' has invalid flag value: {}",
                area.name,
                area.flag
            );
        }
    }

    #[test]
    fn test_specific_known_areas() {
        // Test some specific well-known areas from the AREAS array

        // Aston (first area)
        let aston = &AREAS[0];
        assert_eq!(aston.name, "Aston");
        assert_eq!(aston.flag, 0);
        assert!(aston.contains(500, 450)); // Should be inside Aston

        // Find Lizard Temple
        let lizard_temple = AREAS.iter().find(|a| a.name == "Lizard Temple");
        assert!(lizard_temple.is_some());
        let lizard_temple = lizard_temple.unwrap();
        assert_eq!(lizard_temple.flag, 1);

        // Find Temple Street
        let temple_street = AREAS.iter().find(|a| a.name == "Temple Street");
        assert!(temple_street.is_some());
        let temple_street = temple_street.unwrap();
        assert_eq!(temple_street.flag, 2);
    }

    #[test]
    fn test_area_coordinate_ranges() {
        // Test that area coordinates are within reasonable game world bounds
        for area in AREAS.iter() {
            // Assuming the game world has reasonable coordinate limits
            assert!(
                area.x1 >= 0,
                "Area '{}' has negative x1: {}",
                area.name,
                area.x1
            );
            assert!(
                area.y1 >= 0,
                "Area '{}' has negative y1: {}",
                area.name,
                area.y1
            );
            assert!(
                area.x2 < 2000,
                "Area '{}' has very large x2: {}",
                area.name,
                area.x2
            );
            assert!(
                area.y2 < 2000,
                "Area '{}' has very large y2: {}",
                area.name,
                area.y2
            );
        }
    }
}
