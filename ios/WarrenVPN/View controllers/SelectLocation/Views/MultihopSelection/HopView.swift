import WarrenTypes
import SwiftUI

struct HopView: View {
    let hop: Hop
    let isSelected: Bool
    let onIconPositionChange: (CGRect) -> Void

    var body: some View {
        HStack {
            let name =
                if let location = hop.selectedLocation {
                    if let automaticLocationCountry = location.asAutomaticLocationNode?.locationInfo?.first {
                        String(
                            format: NSLocalizedString(
                                "%@ (%@)",
                                comment: "Selected location name, with country in parentheses"
                            ),
                            location.name,
                            automaticLocationCountry
                        )
                    } else {
                        location.name
                    }
                } else {
                    "Select location"
                }

            hop.icon
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(width: 18)
                .accessibilityHidden(true)
                .capturePosition(
                    in: .multihopSelection
                ) { position in
                    onIconPositionChange(position)
                }
            Text(LocalizedStringKey(name))
                .lineLimit(nil)
                .fixedSize(horizontal: false, vertical: true)
            Spacer()
        }
        .font(.warrenSmallSemiBold)
        .foregroundStyle(
            isSelected
                ? Color.warrenTextPrimary
                : Color.warrenTextSecondary
        )
        .padding(8)
    }
}

#Preview {
    HopView(
        hop: Hop(
            multihopContext: .entry,
            multihopState: .whenNeeded,
            selectedLocation: .init(name: "Sweden", code: "se"),
            filterCount: 1
        ),
        isSelected: true,
        onIconPositionChange: { _ in }
    )
    .background(Color.MullvadList.Item.child3)
}
