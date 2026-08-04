import SwiftUI

struct FloatingSearchBar: View {
    @Binding var searchText: String
    @Binding var isExpanded: Bool
    var isFocused: FocusState<Bool>.Binding

    @Namespace private var animation

    private enum AnimationID {
        case searchIcon
        case searchBackground
    }

    var body: some View {
        HStack {
            if isExpanded {
                HStack(spacing: 8) {
                    searchIcon
                    TextField(
                        "Search location or server",
                        text: $searchText,
                        prompt: Text("Search location or server")
                            .foregroundColor(.MullvadTextField.inputPlaceholder)
                    )
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                    .focused(isFocused)
                    .foregroundColor(.MullvadTextField.textInput)
                    .onSubmit {
                        if searchText.isEmpty {
                            withAnimation {
                                isExpanded = false
                                isFocused.wrappedValue = false
                            }
                        }
                    }
                    if !searchText.isEmpty {
                        Button {
                            searchText = ""
                        } label: {
                            Image.warrenIconCross
                                .foregroundColor(.warrenTextSecondary)
                        }
                        .accessibilityLabel(Text("Clear search"))
                    }
                }
                .padding(.horizontal, 8)
                .frame(height: 48)
                .background {
                    RoundedRectangle(cornerRadius: 28)
                        .fill(Color.warrenContainerBackground)
                        .matchedGeometryEffect(id: AnimationID.searchBackground, in: animation)
                }
                .accessibilityAddTraits(.isSearchField)
                .accessibilityIdentifier(.selectLocationSearchTextField)

                Button {
                    searchText = ""
                    withAnimation {
                        isExpanded = false
                        isFocused.wrappedValue = false
                    }
                } label: {
                    Image.warrenIconCross
                        .foregroundColor(.warrenTextPrimary)
                        .frame(width: 48, height: 48)
                        .background(Color.warrenContainerBackground)
                        .clipShape(Circle())
                }
                .accessibilityLabel(Text("Close search"))
                .accessibilityIdentifier(.closeSearchButton)
                .transition(.opacity)
            } else {
                Spacer()
                Button {
                    withAnimation {
                        isExpanded = true
                    }
                } label: {
                    searchIcon
                        .frame(width: 48, height: 48)
                        .background {
                            RoundedRectangle(cornerRadius: 28)
                                .fill(Color.warrenContainerBackground)
                                .matchedGeometryEffect(id: AnimationID.searchBackground, in: animation)
                        }
                }
                .accessibilityLabel(Text("Search locations"))
                .accessibilityIdentifier(.selectLocationSearchTextField)
            }
        }
        .onChange(of: isExpanded) { _, expanded in
            if expanded {
                isFocused.wrappedValue = true
            }
        }
        // Prevents the keyboard safe area animation from compounding with the bar's
        // layout animation, which otherwise causes a visible bounce on the text field.
        .transformEffect(.identity)
    }

    private var searchIcon: some View {
        Image.warrenIconSearch
            .foregroundColor(.warrenTextPrimary)
            .matchedGeometryEffect(id: AnimationID.searchIcon, in: animation)
    }
}
