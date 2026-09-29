package com.warrenbrowse.vpn.lib.ui.theme.dimensions

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

data class Dimensions(
    val accountRowMinHeight: Dp = 48.dp,
    val accountRowSpacing: Dp = 24.dp,
    val bottomPadding: Dp = 4.dp,
    val buttonHeight: Dp = 44.dp,
    val buttonSpacing: Dp = 8.dp,
    // Desktop CardButton: 32 px tall, radius 6, the location and shuffle
    // buttons 4 px apart, the shuffle 40 px wide.
    val cardButtonHeight: Dp = 32.dp,
    val cardButtonRadius: Dp = 6.dp,
    val cardButtonRowGap: Dp = 4.dp,
    // Desktop BetaBadge overlay: one line in a 36.5 px card, 13.5 px under the
    // header and 15 px in from the edge, its pill 21 px tall.
    val betaBannerMarginTop: Dp = 13.5.dp,
    val betaBannerMarginStart: Dp = 15.dp,
    val betaBannerHeight: Dp = 36.5.dp,
    val betaBannerPaddingStart: Dp = 8.dp,
    val betaBannerPaddingEnd: Dp = 12.dp,
    val betaBannerGap: Dp = 8.dp,
    val betaBannerRadius: Dp = 12.dp,
    val betaBannerShadowOffsetY: Dp = 2.dp,
    val betaBannerShadowBlur: Dp = 8.dp,
    val betaPillHeight: Dp = 21.dp,
    val betaPillPaddingHorizontal: Dp = 8.dp,
    val betaPillRadius: Dp = 6.dp,
    val cellEndPadding: Dp = 16.dp,
    val cellFooterTopPadding: Dp = 4.dp,
    val cellHeight: Dp = 56.dp,
    val cellHeightTwoRows: Dp = 72.dp,
    val cellStartPadding: Dp = 16.dp,
    val cellVerticalSpacing: Dp = 24.dp,
    val chipSpace: Dp = 8.dp,
    val circularProgressBarLargeSize: Dp = 40.dp,
    val circularProgressBarLargeStrokeWidth: Dp = 8.dp,
    val circularProgressBarMediumSize: Dp = 32.dp,
    val circularProgressBarMediumStrokeWidth: Dp = 4.dp,
    val circularProgressBarSmallSize: Dp = 24.dp,
    val circularProgressBarSmallStrokeWidth: Dp = 4.dp,
    val connectButtonExtraPadding: Dp = 4.dp,
    val connectionCardMaxWidth: Dp = 480.dp,
    // Desktop ConnectionPanel: an opaque card of radius 16, 14 px from the
    // sides and 6 px above the footer, its blocks 10.5 px apart, with the
    // 22 px expand chevron box around an 18 px glyph.
    val connectionCardRadius: Dp = 16.dp,
    val connectionCardHorizontalPadding: Dp = 20.dp,
    val connectionCardMarginHorizontal: Dp = 14.dp,
    val connectionCardMarginBottom: Dp = 6.dp,
    val connectionCardBlockGap: Dp = 10.5.dp,
    val connectionCardShadowOffsetY: Dp = 5.dp,
    val connectionCardShadowBlur: Dp = 16.dp,
    val connectionCardChevronSize: Dp = 22.dp,
    val connectionCardChevronIconSize: Dp = 18.dp,
    // Desktop Hostname: 2 px under the location line.
    val hostnameGapAbove: Dp = 2.dp,
    // Desktop FeatureIndicator: 5.5 x 8 padding, radius 7, pills stacked 2 px
    // apart and 4 px above the card.
    val chipVerticalPadding: Dp = 5.5.dp,
    val chipHorizontalPadding: Dp = 8.dp,
    val chipCornerRadius: Dp = 7.dp,
    val chipStackGap: Dp = 2.dp,
    val chipsToCardGap: Dp = 4.dp,
    val chipShadowOffsetY: Dp = 1.5.dp,
    val chipShadowBlur: Dp = 5.dp,
    // The chip stack turns the minimum-interactive row off (0.dp is Material's
    // documented "no enforcement"), so a chip's layout height is its pill and
    // the gap above is the gap the user sees. With the row left at 48 dp the
    // pills measured 48 dp centre to centre on a 1080x2400 screen, which is
    // what the stack was reported for. DesignParityTest carries why that is
    // still an honest target.
    val chipInteractiveMinSize: Dp = 0.dp,
    // Desktop AppMainFooter: 7 x 16.
    val footerVerticalPadding: Dp = 7.dp,
    // Desktop NotificationBanner: radius 14, 2 px status edge, 10 12 10 16 padding.
    val notificationBannerRadius: Dp = 14.dp,
    val notificationBannerEdge: Dp = 2.dp,
    val notificationBannerVerticalPadding: Dp = 10.dp,
    val notificationBannerElevation: Dp = 8.dp,
    // Desktop DialogPopup: radius 12 on the darkBlue container.
    val dialogCornerRadius: Dp = 12.dp,
    // Desktop ConnectionPanel: 20 px all round.
    val connectionCardVerticalPadding: Dp = 20.dp,
    // Desktop ConnectionStatus: 12 px between the eye and the text, 1 px
    // between title and subtitle, 12 px between the chevron and the flag; the
    // eye is an 18 px glyph in a 34 px well of radius 8.
    val connectionStatusGap: Dp = 12.dp,
    val connectionStatusTextGap: Dp = 1.dp,
    val connectionStatusTrailingGap: Dp = 12.dp,
    val connectionStatusWellSize: Dp = 34.dp,
    val connectionStatusWellRadius: Dp = 8.dp,
    val connectionStatusIconSize: Dp = 18.dp,
    // Desktop CurrentCountryFlag: a 22 px round flag with a hairline ring.
    val countryFlagSize: Dp = 22.dp,
    val deleteIconSize: Dp = 24.dp,
    val dialogIconHeight: Dp = 48.dp,
    val fabSpacing: Dp = 16.dp, // Copied from the private val FabSpacing in Scaffold.kt
    val formTextFieldMinHeight: Dp = 56.dp,
    val formVerticalSpacingGroups: Dp = 32.dp,
    val formVerticalSpacingInsideGroups: Dp = 16.dp,
    val hopIconSize: Dp = 24.dp,
    val hopIconVerticalInternalPadding: Dp = 2.dp,
    val hopRadius: Dp = 12.dp,
    val hopSelectorErrorStartPadding: Dp = 28.dp,
    val indentedCellStartPadding: Dp = 48.dp,
    val indicatorPadding: Dp = 4.dp,
    val indicatorSize: Dp = 8.dp,
    val largePadding: Dp = 32.dp,
    val largeSpacer: Dp = 24.dp,
    val listIconSize: Dp = 24.dp,
    val listItemDivider: Dp = 1.dp,
    val locationHintIconSize: Dp = 18.dp,
    val locationHintInternalPadding: Dp = 2.dp,
    val mediumIconSize: Dp = 32.dp,
    val mediumPadding: Dp = 16.dp,
    val mediumSpacer: Dp = 16.dp,
    val miniPadding: Dp = 4.dp,
    val multihopSelectorPanelRadius: Dp = 16.dp,
    val notificationBannerEndPadding: Dp = 12.dp,
    val notificationBannerStartPadding: Dp = 16.dp,
    val notificationEndIconPadding: Dp = 4.dp,
    // This is according to the design, should be updated in the design to standard size
    val notificationStatusIconSize: Dp = 10.dp,
    val obfuscationNavigationBoxWidth: Dp = 56.dp,
    val outLineButtonBorderWidth: Dp = 1.dp,
    val orDivierMinHeight: Dp = 48.dp,
    val privacyPolicyIconSize: Dp = 16.dp,
    val problemReportTextFieldMinHeight: Dp = 220.dp,
    val reconnectButtonMinInteractiveComponentSize: Dp = 40.dp,
    val reconnectButtonDivider: Dp = 1.dp,
    val relayCirclePadding: Dp = 5.dp,
    val relayCircleSize: Dp = 16.dp,
    val relayItemCornerRadius: Dp = 16.dp,
    // These two should be consolidated into one value when OK'd with design.
    val screenBottomMargin: Dp = 16.dp,
    val screenBottomMarginNew: Dp = 24.dp,
    val screenTopMargin: Dp = 24.dp,
    val searchFieldHeight: Dp = 42.dp,
    val searchFieldHeightExpanded: Dp = 72.dp,
    val searchFieldHorizontalPadding: Dp = 20.dp,
    val searchIconSize: Dp = 24.dp,
    val shuffleButtonWidth: Dp = 40.dp,
    val selectableCellTextMargin: Dp = 8.dp,
    val settingsDetailsImageMaxWidth: Dp = 480.dp,
    // These two should be consolidated into one value when OK'd with design.
    val sideMargin: Dp = 24.dp,
    val sideMarginNew: Dp = 16.dp,
    val smallIconSize: Dp = 16.dp,
    val smallPadding: Dp = 8.dp,
    val smallSpacer: Dp = 8.dp,
    // The brand lockup on the splash, no-daemon and forced-update screens: tall
    // enough to be the screen's subject, narrow enough for a 360 dp phone with
    // the side margins (the lockup is about three times as wide as it is tall).
    val splashLockupHeight: Dp = 72.dp,
    // The hairline of the opaque connect-screen surfaces: the card, its
    // buttons, the chips, the beta banner and the flag ring.
    val surfaceBorderWidth: Dp = 0.5.dp,
    val switchIconSize: Dp = 24.dp,
    // The desktop ShuffleButton is 40 px wide for a pointer; a finger gets the
    // 48 dp floor, and the location button 1 dp away wins any tap left of it.
    val switchLocationRetryMinWidth: Dp = 48.dp,
    val thinBorderWidth: Dp = 1.dp,
    val tinyPadding: Dp = 4.dp,
    // Desktop MainHeader: 32 px icons, 48 px lockup.
    val topBarActionIconSize: Dp = 32.dp,
    val topBarLockupHeight: Dp = 48.dp,
    val tvDrawerHeaderStartPadding: Dp = 12.dp,
    val tvDrawerHeaderWithFocusStartPadding: Dp = 16.dp,
    val tvDrawerHorizontalPadding: Dp = 12.dp,
    val verticalSpace: Dp = 16.dp,
)

val defaultDimensions = Dimensions()
// Add more configurations here if needed
