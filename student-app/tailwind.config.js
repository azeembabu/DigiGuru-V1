/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        dg: {
          50: '#ECFDF5',
          100: '#D1FAE5',
          200: '#A7F3D0',
          300: '#6EE7B7',
          400: '#34D399',
          500: '#10B981',
          600: '#059669',
          700: '#047857',
          800: '#065F46',
          900: '#0E4B3A',
          950: '#022C22',
        },
        ink: {
          900: '#0F172A',
          700: '#334155',
          500: '#64748B',
          400: '#94A3B8',
        },
      },
      fontFamily: {
        sans: ['Inter', 'Noto Sans Malayalam', 'ui-sans-serif', 'system-ui', 'sans-serif'],
        display: ['Inter', 'Noto Sans Malayalam', 'ui-sans-serif', 'system-ui', 'sans-serif'],
      },
      borderRadius: {
        '4xl': '2rem',
      },
      boxShadow: {
        glass: '0 8px 32px rgba(14,75,58,0.08), 0 2px 8px rgba(14,75,58,0.06)',
        'glass-lg': '0 20px 60px rgba(14,75,58,0.12), 0 4px 20px rgba(14,75,58,0.08)',
        soft: '0 1px 3px rgba(0,0,0,0.06), 0 8px 24px rgba(0,0,0,0.04)',
      },
      backdropBlur: {
        glass: '16px',
      },
    },
  },
  plugins: [],
};
