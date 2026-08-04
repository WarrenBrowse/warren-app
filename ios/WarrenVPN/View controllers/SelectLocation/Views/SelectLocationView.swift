import WarrenSettings
import WarrenTypes
import SwiftUI

struct SelectLocationView<ViewModel>: View where ViewModel: SelectLocationViewModel {
    @ObservedObject var viewModel: ViewModel
    @State private var headerIsExpandedForExit: Bool = false
    @State private var disablingRecentConnectionsAlert: MullvadAlert?
    @FocusState private var focusSearchField: Bool
    @State private var isSearchExpanded: Bool = false
    @State private var headerHeight: CGFloat = 0
    @State private var floatingBarHeight: CGFloat = 0
    @ScaledMetric(relativeTo: .body) private var listBottomInset: CGFloat = 56

    private var headerIsExpanded: Bool {
        headerIsExpandedForExit
    }

    // The exit list is always searchable now that the entry hop (whose
    // when-needed info card suppressed the field) is no longer shown.
    private var showSearchField: Bool {
        true
    }

    var body: some View {
        // Simply animating the MultihopSelectionView while scrolling leads to a slow
        // down of the scrolling during the animation. Instead of changing the size of the scroll
        // view, the top margin of the content is changed which solves the animation issues.
        ZStack(alignment: .topLeading) {
            VStack(spacing: 16) {
                // Exit-only surface: the entry hop is never user-selectable on
                // the Warren network (the circuit selector implies the entry
                // from the exit), so the select-location screen shows a single
                // exit hop rather than an entry/exit switcher.
                MultihopSelectionView(
                    hops: [
                        Hop(
                            multihopContext: .exit,
                            multihopState: viewModel.multihopState,
                            selectedLocation: viewModel.exitContext.selectedLocation,
                            filterCount: viewModel.exitContext.filter.count
                        )
                    ],
                    selectedMultihopContext: $viewModel.multihopContext,
                    isExpanded: headerIsExpanded
                )
                .padding(.horizontal, 16)
            }
            .padding(.vertical)
            .background(Color.warrenDarkBackground)
            .zIndex(1)
            .sizeOfView { size in
                withAnimation {
                    headerHeight = size.height
                }
            }
            VStack {
                // Prevent scroll content from touching navigation bar to avoid a change of appearence
                // see `UINavigationBar+Appearance.swift`
                Spacer()
                    .frame(height: 1)
                Group {
                    ExitLocationView(
                        viewModel: viewModel,
                        context: $viewModel.exitContext,
                        onScrollVisibilityChange: {
                            expandHeader in
                            withAnimation {
                                headerIsExpandedForExit = expandHeader
                            }
                        }
                    )
                }
                .simultaneousGesture(
                    DragGesture(minimumDistance: 50)
                        .onChanged { _ in
                            focusSearchField = false
                        }
                )
                .environment(\.dismissSearchFocus, { focusSearchField = false })
                .geometryGroup()
                // Adds margin to the top of the scroll content. The scroll views size stays untouched
                // which seems to be the solution to animation issues.
                .contentMargins(.top, headerHeight - 1)
                .contentMargins(.bottom, showSearchField ? floatingBarHeight + listBottomInset : 0)
                .zIndex(0)
            }
        }
        .overlay(alignment: .bottom) {
            FloatingSearchBar(
                searchText: $viewModel.searchText,
                isExpanded: $isSearchExpanded,
                isFocused: $focusSearchField
            )
            .showIf(showSearchField)
            .padding(.horizontal, 24)
            .padding(.bottom, 16)
            .sizeOfView { floatingBarHeight = $0.height }
            .accessibilitySortPriority(1)
        }
        .onChange(of: showSearchField) { _, newValue in
            if !newValue {
                isSearchExpanded = false
                viewModel.searchText = ""
            }
        }
        .animation(.default, value: isSearchExpanded)
        .animation(.default, value: showSearchField)
        .animation(.default, value: viewModel.multihopContext)
        .animation(.default, value: viewModel.isMultihopActive)
        .animation(.default, value: viewModel.isRecentsEnabled)
        .background(Color.warrenDarkBackground)
        .navigationTitle("Select location")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(
                placement: .topBarTrailing,
                content: {
                    Button("Done") {
                        viewModel.didFinish()
                    }
                    .foregroundStyle(Color.warrenTextPrimary)
                    .accessibilityIdentifier(.closeSelectLocationButton)
                }
            )
            ToolbarItem(
                placement: .topBarLeading,
                content: {
                    Menu {
                        Picker(selection: $viewModel.multihopState) {
                            ForEach(MultihopState.allCases, id: \.self) { state in
                                HStack {
                                    Text(state.description)
                                    state.icon
                                        .renderingMode(.template)
                                }
                                .accessibilityIdentifier(.multihopState(state.description))
                            }
                        } label: {
                            Text("Multihop mode")
                            Text(viewModel.multihopState.description)
                        }
                        .pickerStyle(MenuPickerStyle())
                        .accessibilityIdentifier(.multihopMenuPicker)

                        Button {
                            if viewModel.isRecentsEnabled {
                                disablingRecentConnectionsAlert = MullvadAlert(
                                    type: .warning,
                                    messages: ["Disabling recents will also clear history."],
                                    actions: [
                                        MullvadAlert.Action(
                                            type: .danger,
                                            title: "Disable",
                                            identifier: AccessibilityIdentifier.disableRecentConnectionsButton,
                                            handler: {
                                                disablingRecentConnectionsAlert = nil
                                                viewModel.toggleRecents()
                                            }
                                        ),
                                        MullvadAlert.Action(
                                            type: .default,
                                            title: "Cancel",
                                            handler: {
                                                disablingRecentConnectionsAlert = nil
                                            }
                                        ),
                                    ]
                                )

                            } else {
                                viewModel.toggleRecents()
                            }

                        } label: {
                            HStack {
                                Text(viewModel.isRecentsEnabled ? "Disable recents" : "Enable recents")
                                viewModel.isRecentsEnabled
                                    ? Image.warrenIconDisableRecents
                                        .renderingMode(.template)
                                    : Image.warrenIconEnableRecents
                                        .renderingMode(.template)
                            }
                        }
                        .accessibilityIdentifier(.recentConnectionsToggleButton)

                        Button {
                            viewModel.manuallyFetchRelayList()
                        } label: {
                            HStack {
                                Text("Update server list")
                                Image.warrenIconReload
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle.fill")
                            .foregroundStyle(Color.warrenTextPrimary)
                            .accessibilityIdentifier(.selectLocationToolbarMenu)
                    }
                }
            )
        }
        .warrenAlert(item: $disablingRecentConnectionsAlert)
    }
}

#Preview {
    Text("")
        .sheet(isPresented: .constant(true)) {
            NavigationView {
                SelectLocationView(
                    viewModel: MockSelectLocationViewModel()
                )
            }
        }
}
