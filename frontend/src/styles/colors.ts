/**
 * Color utilities using Blueprint.js Colors constant
 *
 * This file provides access to Blueprint's color system in a type-safe way.
 * Use these instead of hardcoding color values in your components.
 *
 * @see https://blueprintjs.com/docs/#core/colors
 */

import { Colors } from '@blueprintjs/core';

/**
 * Blueprint v6 Core Colors
 * Use these for intent-based UI elements (buttons, callouts, icons)
 */
export const IntentColors = {
  // Primary (Blue)
  primary: Colors.BLUE3,
  primaryLight: Colors.BLUE4,
  primaryDark: Colors.BLUE2,

  // Success (Green)
  success: Colors.GREEN3,
  successLight: Colors.GREEN4,
  successDark: Colors.GREEN2,

  // Warning (Orange)
  warning: Colors.ORANGE3,
  warningLight: Colors.ORANGE4,
  warningDark: Colors.ORANGE2,

  // Danger (Red)
  danger: Colors.RED3,
  dangerLight: Colors.RED4,
  dangerDark: Colors.RED2,
} as const;

/**
 * Blueprint v6 Gray Scale
 * Use these for main UI frame: containers, headers, sections, boxes
 */
export const GrayScale = {
  // Dark colors (for dark theme)
  black: Colors.BLACK,
  darkGray1: Colors.DARK_GRAY1,
  darkGray2: Colors.DARK_GRAY2,
  darkGray3: Colors.DARK_GRAY3,
  darkGray4: Colors.DARK_GRAY4,
  darkGray5: Colors.DARK_GRAY5,
  gray1: Colors.GRAY1,
  gray2: Colors.GRAY2,
  gray3: Colors.GRAY3,
  gray4: Colors.GRAY4,
  gray5: Colors.GRAY5,

  // Light colors
  lightGray1: Colors.LIGHT_GRAY1,
  lightGray2: Colors.LIGHT_GRAY2,
  lightGray3: Colors.LIGHT_GRAY3,
  lightGray4: Colors.LIGHT_GRAY4,
  lightGray5: Colors.LIGHT_GRAY5,
  white: Colors.WHITE,
} as const;

/**
 * Extended Colors for data visualizations
 * Use these for charts, graphs, and data representation
 */
export const DataColors = {
  // Blue family
  blue1: Colors.BLUE1,
  blue2: Colors.BLUE2,
  blue3: Colors.BLUE3,
  blue4: Colors.BLUE4,
  blue5: Colors.BLUE5,

  // Green family
  green1: Colors.GREEN1,
  green2: Colors.GREEN2,
  green3: Colors.GREEN3,
  green4: Colors.GREEN4,
  green5: Colors.GREEN5,

  // Orange family
  orange1: Colors.ORANGE1,
  orange2: Colors.ORANGE2,
  orange3: Colors.ORANGE3,
  orange4: Colors.ORANGE4,
  orange5: Colors.ORANGE5,

  // Red family
  red1: Colors.RED1,
  red2: Colors.RED2,
  red3: Colors.RED3,
  red4: Colors.RED4,
  red5: Colors.RED5,

  // Violet family
  violet1: Colors.VIOLET1,
  violet2: Colors.VIOLET2,
  violet3: Colors.VIOLET3,
  violet4: Colors.VIOLET4,
  violet5: Colors.VIOLET5,

  // Indigo family
  indigo1: Colors.INDIGO1,
  indigo2: Colors.INDIGO2,
  indigo3: Colors.INDIGO3,
  indigo4: Colors.INDIGO4,
  indigo5: Colors.INDIGO5,
} as const;

/**
 * Anduril-specific theme colors mapped to Blueprint
 * These maintain the Anduril aesthetic while using Blueprint's color system
 */
export const AndurilTheme = {
  // Electric blue accent (matches your --accent color)
  accent: Colors.BLUE3,
  accentHover: Colors.BLUE4,

  // Text colors
  textPrimary: Colors.LIGHT_GRAY5,  // #F6F7F9
  textMuted: Colors.GRAY5,           // #ABB3BF
  textDisabled: Colors.GRAY3,        // #5F6B7C

  // Background colors
  bgDark: Colors.DARK_GRAY1,         // #1C2127
  bgDarker: Colors.BLACK,            // #111418
  bgCard: Colors.DARK_GRAY2,         // #252A31

  // Border colors
  border: Colors.DARK_GRAY3,         // #2F343C
  borderLight: Colors.DARK_GRAY4,    // #383E47
} as const;

/**
 * Example usage in components:
 *
 * import { IntentColors, GrayScale, AndurilTheme } from '@/styles/colors';
 *
 * // In a styled component or inline style:
 * <div style={{ color: AndurilTheme.textPrimary, backgroundColor: AndurilTheme.bgDark }}>
 *   <Icon icon="tick" style={{ color: IntentColors.success }} />
 * </div>
 */
