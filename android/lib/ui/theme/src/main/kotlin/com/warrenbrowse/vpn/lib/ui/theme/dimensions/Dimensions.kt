package com.warrenbrowse.vpn.lib.ui.theme.dimensions

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

data class Dimensions(
    val accountRowMinHeight: Dp = 48.dp,
    val accountRowSpacing: Dp = 24.dp,
    val bottomPadding: Dp = 4.dp,
    val buttonHeight: Dp = 44.dp,
    val buttonSpacing: Dp = 8.dp,
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
    // Desktop ConnectionPanel: radius16 on the glass card.
    val connectionCardRadius: Dp = 16.dp,
    // Desktop FeatureIndicator: 2 x 8 padding, radius8. The desktop stacks its
    // 22 px pills 5 px apart; here every chip sits in a 48 dp touch row, so the
    // gap is between rows, and the pills already fall 26 dp apart at 0.
    val chipVerticalPadding: Dp = 2.dp,
    val chipHorizontalPadding: Dp = 8.dp,
    val chipCornerRadius: Dp = 8.dp,
    val chipStackGap: Dp = 0.dp,
    // Desktop AppMainFooter: 7 x 16.
    val footerVerticalPadding: Dp = 7.dp,
    // Desktop NotificationBanner: radius 14, 2 px status edge, 10 12 10 16 padding.
    val notificationBannerRadius: Dp = 14.dp,
    val notificationBannerEdge: Dp = 2.dp,
    val notificationBannerVerticalPadding: Dp = 10.dp,
    val notificationBannerElevation: Dp = 8.dp,
    // Desktop DialogPopup: radius 12 on the darkBlue container.
    val dialogCornerRadius: Dp = 12.dp,
    // Desktop ConnectionPanel: 14 px vertical, 16 px horizontal.
    val connectionCardVerticalPadding: Dp = 14.dp,
    // Desktop ConnectionStatus: 12 px between the eye and the text.
    val connectionStatusGap: Dp = 12.dp,
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
    val selectableCellTextMargin: Dp = 8.dp,
    val settingsDetailsImageMaxWidth: Dp = 480.dp,
    // These two should be consolidated into one value when OK'd with design.
    val sideMargin: Dp = 24.dp,
    val sideMarginNew: Dp = 16.dp,
    val smallIconSize: Dp = 16.dp,
    val smallPadding: Dp = 8.dp,
    val smallSpacer: Dp = 8.dp,
    val splashLogoSize: Dp = 120.dp,
    // Required to get the logo to look visually correct
    val splashLogoTextHeight: Dp = 18.dp,
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
    // Required to get the logo to look visually correct on TV
    val tvMullvadLogoTextHeight: Dp = 13.dp,
    // Required to get the logo to look visually correct on TV
    val tvMullvadLogoTextStartPadding: Dp = 6.dp,
    val verticalSpace: Dp = 16.dp,
)

val defaultDimensions = Dimensions()
// Add more configurations here if needed
