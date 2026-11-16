export enum FontFamilyTokens {
  // Instrument Sans has no Cyrillic: Open Sans draws Russian and Ukrainian, and the
  // Noto faces the two scripts neither covers.
  sans = '"Instrument Sans", "Open Sans", "Noto Sans Myanmar", "Noto Sans Thai", sans-serif',
  mono = '"JetBrains Mono", ui-monospace, Menlo, Consolas, monospace',
  // Brand display face for the WARREN wordmark only (Logo.tsx), not the UI text.
  nunito = '"Nunito", sans-serif',
}

export enum FontWeightTokens {
  regular = 400,
  semiBold = 600,
  bold = 700,
}

export enum FontSizeTokens {
  big = '32px',
  large = '24px',
  medium = '18px',
  small = '14px',
  tiny = '12px',
  mini = '10px',
}

export enum LineHeightTokens {
  big = '34px',
  large = '28px',
  medium = '24px',
  small = '20px',
  tiny = '18px',
  mini = '15px',
}
