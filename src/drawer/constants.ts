export const canvasWidth = 1024, canvasHeight = 768;

// Define color constants
export const COLORS = {
    TRANSPARENT: 'transparent',
    RED: '#ff0000',
    GREEN: '#00ff00',
    YELLOW: '#ffff00',
    BLUE: '#0000ff',
    BLACK: '#000000'
};

// Single color mapping with bidirectional lookup methods
export const COLOR_MAP = {
    'transparent': COLORS.TRANSPARENT,
    'rot': COLORS.RED,
    'grün': COLORS.GREEN,
    'gelb': COLORS.YELLOW,
    'blau': COLORS.BLUE,
    'schwarz': COLORS.BLACK
};

// Helper functions for bidirectional lookup
export function getColorName(colorValue: string): string {
    // Manual iteration instead of using Object.entries
    for (const name in COLOR_MAP) {
        if (COLOR_MAP.hasOwnProperty(name)) {
            if (COLOR_MAP[name] === colorValue) {
                return name;
            }
        }
    }
    return 'unknown';
}

export function getColorValue(colorName: string): string {
    return COLOR_MAP[colorName] || COLORS.BLACK;
}