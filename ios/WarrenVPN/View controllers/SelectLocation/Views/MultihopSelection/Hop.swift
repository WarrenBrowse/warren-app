import WarrenSettings
import WarrenTypes
import SwiftUI

struct Hop {
    let multihopContext: MultihopContext
    let multihopState: MultihopState
    let selectedLocation: LocationNode?
    let filterCount: Int
    var noMatchFound: NoMatchFoundReason? {
        if let selectedLocation {
            if let customListLocation = selectedLocation as? CustomListLocationNode {
                if customListLocation.children.isEmpty {
                    return .customListEmpty
                }
            }
            if !selectedLocation.isActive {
                return .selectionInactive
            } else {
                return nil
            }
        }
        return .noSelection
    }

    var icon: Image {
        return if noMatchFound != nil {
            .warrenIconError
        } else {
            switch multihopContext {
            case .entry:
                if selectedLocation is AutomaticLocationNode {
                    multihopState.icon
                } else {
                    .warrenServer
                }
            case .exit:
                .warrenLocation
            }
        }
    }

    enum NoMatchFoundReason {
        // Previous selection is no longer valid with filters, settings or simply disappeared from the relay list
        case noSelection
        // A location is inactive when all containing relays are inactive (offline)
        case selectionInactive
        // A selected custom list has no locations with current filters settings or is simply empty.
        case customListEmpty

        // Could me more detailed in the future
        var description: LocalizedStringKey {
            "No servers match your settings, try changing server or other settings."
        }
    }
}
