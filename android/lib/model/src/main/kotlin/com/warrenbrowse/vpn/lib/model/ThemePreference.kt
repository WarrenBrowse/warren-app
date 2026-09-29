package com.warrenbrowse.vpn.lib.model

/** The palette the screens are actually painted in. */
enum class ColorTheme {
    DARK,
    LIGHT,
}

/**
 * The theme the user asked for: follow the device, or one of the two palettes whatever the device
 * says. Mirrors the desktop `shared/theme.ts`, stored values included.
 */
enum class ThemePreference(val storageValue: String) {
    SYSTEM("system"),
    DARK("dark"),
    LIGHT("light");

    /**
     * Dark is the house palette, so a device that states no theme ([systemTheme] null) paints
     * dark.
     */
    fun resolve(systemTheme: ColorTheme?): ColorTheme =
        when (this) {
            SYSTEM -> systemTheme ?: ColorTheme.DARK
            DARK -> ColorTheme.DARK
            LIGHT -> ColorTheme.LIGHT
        }

    companion object {
        // A value a later version wrote, or none at all, follows the device.
        fun parse(value: String?): ThemePreference =
            entries.firstOrNull { it.storageValue == value } ?: SYSTEM
    }
}
