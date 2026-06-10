import SwiftUI

struct MullvadListSectionHeader: View {
    let title: LocalizedStringKey
    let subtitle: LocalizedStringKey?

    init(title: LocalizedStringKey, subtitle: LocalizedStringKey? = nil) {
        self.title = title
        self.subtitle = subtitle
    }

    var body: some View {
        HStack {
            Text(title)
                .font(.warrenTiny)
                .foregroundStyle(Color.warrenTextPrimary)
                .layoutPriority(1)
            Rectangle()
                .frame(height: 1)
                .foregroundStyle(Color.warrenTextPrimary.opacity(0.2))
            if let subtitle {
                Text(subtitle)
                    .font(.warrenTiny)
                    .foregroundStyle(Color.warrenTextSecondary)
                    .layoutPriority(1)
            }
        }
        .frame(minHeight: 44, alignment: .center)
        .accessibilityAddTraits(.isHeader)
    }
}

#Preview {
    MullvadListSectionHeader(title: "Custom lists")
}
