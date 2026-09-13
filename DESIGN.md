# Digi Guru — Design System

**Status:** v1 — extracted from the approved hero mockup, ready for implementation in `apps/web`.
**Applies to:** marketing site, auth pages, student dashboard & classroom, admin console.

---

## 0. Mood

Confident, futuristic, a little playful. A near-black "focus" surface with one electric accent
color doing all the talking, plus a cooler secondary accent for structure and depth. This is a
platform that teaches with AI and voice — the palette should read as *digital tutor*, not
*corporate LMS*: high contrast, generous whitespace, rounded-but-sharp geometry, soft glows instead
of hard shadows.

Three words to design against: **dark, electric, calm.** Dark surfaces do the heavy lifting; lime
is used sparingly and only where it means "go" or "look here"; indigo softens everything else.

---

## 1. Color System

### 1.1 Palette

| Token | Hex | Role |
|---|---|---|
| `ink-950` | `#0A0C16` | Primary dark surface — hero, classroom, focus mode |
| `ink-900` | `#12141F` | Elevated dark surface — cards, panels, nav on dark |
| `ink-800` | `#1C1F2E` | Borders / dividers on dark surfaces |
| `ink-700` | `#2A2E42` | Hover state for dark surface elements |
| `lime-300` | `#C7F26B` | Lime hover / highlighted text |
| `lime-400` | `#A6E635` | **Primary brand accent** — CTAs, active states, key icons |
| `lime-500` | `#8FC91E` | Lime pressed / active state |
| `lime-950` | `#12190A` | Near-black green, for a lime-tinted dark chip background |
| `indigo-300` | `#9EA3F2` | Indigo hover / light text on dark |
| `indigo-400` | `#6E74E8` | **Secondary accent** — badges, glows, links, informational state |
| `indigo-500` | `#5A5FD1` | Indigo pressed / deep accent |
| `indigo-950` | `#171A33` | Indigo-tinted dark surface (badge fill on `ink-950`) |
| `lavender-50` | `#F4F4FC` | App light-mode background |
| `lavender-100` | `#E4E5F8` | Marketing canvas background, light surface fill |
| `lavender-200` | `#D2D3F0` | Light-mode border / divider |
| `white` | `#FFFFFF` | Headline text on dark, card fill on light |
| `gray-300` | `#B6B9C9` | Muted text on dark (nav links, captions) |
| `gray-500` | `#6C6F82` | Muted text on light (captions, placeholder) |
| `gray-900` | `#15161F` | Body text on light surfaces |
| `success` | `#3FCF7F` | Confirmation states (kept distinct from brand lime) |
| `danger` | `#F1503D` | Errors, destructive actions, safety-incident indicators |
| `warning` | `#F5B84A` | Quota warnings (e.g. the 18-minute session notice) |

> Lime and indigo are the *only* two saturated hues in the system. Success/danger/warning are
> functional colors, not brand colors — never substitute lime for a "success" state or indigo for
> an "info" state; keep the semantic set separate from the two brand accents.

### 1.2 Gradients & glows

- **Portrait glow** — the soft blob behind a person/photo: `radial-gradient(120% 120% at 30% 20%,
  #6E74E8 0%, #3E3FA8 55%, #171A33 100%)`, placed behind a rounded-square (28px radius) image mask.
- **CTA glow** — a blurred lime ellipse (`lime-400` at 35% opacity, `blur(40px)`) sitting behind a
  primary button on dark surfaces, visible only on hover/focus — never static, it should feel like
  the button is "warming up."
- **Card edge glow (dark mode)** — a 1px inset border in `ink-700` plus a barely-visible top
  highlight (`white` at 4% opacity) to fake soft studio lighting on flat dark cards.

### 1.3 Light / dark surface pairs

| Context | Background | Primary text | Secondary text | Accent for CTAs |
|---|---|---|---|---|
| Marketing hero, classroom, focus mode | `ink-950` | `white` | `gray-300` | `lime-400` |
| Dark cards/panels (nested) | `ink-900` | `white` | `gray-300` | `lime-400` / `indigo-400` |
| App light mode (dashboards, admin) | `lavender-50` | `gray-900` | `gray-500` | `lime-500` (darker, for AA contrast) |
| Light cards | `white` | `gray-900` | `gray-500` | `lime-500` |
| Marketing canvas (outside the product frame) | `lavender-100` | `gray-900` | `gray-500` | n/a |

---

## 2. Typography

| Role | Family | Weight | Notes |
|---|---|---|---|
| Display / headline | **Sora** | 700 (Bold) | Chunky, geometric, slightly rounded terminals — matches the reference headline treatment |
| UI / body | **Inter** | 400–600 | Workhorse text: nav, buttons, forms, body copy |
| Malayalam content | **Noto Sans Malayalam** | 400–700 | Matched x-height/weight to Inter for bilingual harmony — required for the whiteboard per `IMPLEMENTATION_PLAN.md` §6.4 |
| Numerals / stats | Sora, tabular figures | 700 | Stat blocks (`2.5M+`, `50K+`) always use tabular numerals so columns align |

### Scale

| Token | Size / line-height | Use |
|---|---|---|
| `display-lg` | 56px / 1.05 | Hero headline (3-line max, as in the reference) |
| `display-md` | 40px / 1.1 | Section headlines |
| `heading-lg` | 28px / 1.2 | Card / panel titles |
| `heading-md` | 20px / 1.3 | Sub-section titles |
| `body-lg` | 18px / 1.6 | Lead paragraphs |
| `body-md` | 16px / 1.6 | Default body |
| `body-sm` | 14px / 1.5 | Captions, stat labels, nav links |
| `label` | 12px / 1.4, uppercase, +0.04em tracking | Eyebrow labels, badge text |

**Two-tone headline rule.** The reference headline highlights one word in `lime-400` mid-sentence
("Grow up **YOUR** skill in minutes") — reuse this as a standing pattern for hero and section
headlines: one emphasized word or short phrase per headline, lime on dark / `lime-500` on light,
rest of the line in the default text color. Never highlight more than one phrase per headline —
it stops reading as emphasis once overused.

---

## 3. Spacing, Radius & Elevation

| Token | Value |
|---|---|
| `space-1` … `space-16` | 4px base scale: 4, 8, 12, 16, 20, 24, 32, 40, 48, 64px |
| `radius-sm` | 8px — inputs, small chips |
| `radius-md` | 16px — cards, panels |
| `radius-lg` | 28px — hero imagery frame, large feature cards |
| `radius-full` | 999px — pill buttons, avatar badges, nav CTA |

Elevation is glow, not shadow, on dark surfaces (see 1.2); on light surfaces use a conventional soft
shadow: `0 8px 24px rgba(21, 22, 31, 0.08)` for resting cards, `0 12px 32px rgba(21, 22, 31, 0.14)`
for hover/raised state.

---

## 4. Iconography & Imagery

- **Icons**: single-weight line icons (1.5–2px stroke), rounded caps/joins — matches the sparkle
  and lightbulb icons in the reference. Use `lime-400` for icons that indicate an active/available
  action, `indigo-400` for icons that are purely decorative or informational.
- **Photography**: candid, warmly-lit portrait cutouts on a transparent background, composited onto
  the indigo portrait-glow blob (§1.2). Avoid stock-photo stiffness — expressions should be
  mid-action (laughing, gesturing), matching the reference's energy.
- **Decorative badges**: small floating circular chips (40–56px) filled `indigo-950` with a
  centered icon or a curved repeating label (e.g. "GET IN TOUCH •"), used to add motion and depth
  around a hero image — never more than 2–3 per composition, or they compete with the headline.

---

## 5. Core Components

### 5.1 Buttons

| Variant | Fill | Text | Border | Use |
|---|---|---|---|---|
| Primary (dark bg) | `lime-400` | `ink-950` (never white — fails contrast on lime) | none | Main CTA: "Get Started" |
| Primary (light bg) | `lime-500` | `ink-950` | none | Same role in light-mode surfaces |
| Secondary / outline | transparent | `lime-400` | 1.5px `lime-400` | Nav-level CTA: "Register" |
| Ghost / text link | transparent | current text color | none, underline on text | "Learn More ↗" — pair with a small arrow icon, rotate 45° on hover |
| Danger | `danger` | `white` | none | Destructive admin actions only |

All buttons: `radius-full`, 12–16px vertical padding, `body-md` weight 600. Focus state is a 2px
`indigo-300` ring with 2px offset — never rely on color change alone for focus (accessibility).

### 5.2 Badges & stat blocks

- **Quote / stat badge**: circular, `indigo-950` fill, `indigo-400` icon or numeral, paired inline
  with a short label (e.g. "200+ Student Learn Daily") — used for social proof, never for
  navigation.
- **Rating**: `lime-400` filled stars + `body-sm` numeral in `white`/`gray-900` depending on
  surface. Do not recolor the stars per rating value — lime stays constant.
- **Stat row**: bold `display`-weight numeral (`heading-lg`, tabular figures) over a `body-sm`
  `gray-300`/`gray-500` caption, no dividers between columns — separation is whitespace only.

### 5.3 Navigation

Horizontal nav on dark surfaces: wordmark left (accent-colored mark + bold wordmark, see §6),
links centered or left-aligned in `gray-300` at `body-md`, active/hover state transitions to
`white` with no underline, CTA button (outline variant) right-aligned. Sticky on scroll, background
fades from transparent to `ink-950` at 80% opacity with a blur backdrop after ~24px of scroll.

### 5.4 Cards

Dark card: `ink-900` fill, `ink-800` 1px border, `radius-md`, 24px padding, edge-glow per §1.2.
Light card: `white` fill, `lavender-200` 1px border, `radius-md`, soft shadow per §3.

---

## 6. Logo Lockup

Construction: a small accent-colored ring or dot (lime, ~1.5–2× the wordmark's cap-height) directly
preceding a bold wordmark in the display typeface. On dark surfaces the ring is `lime-400` outline
only (transparent center); on light surfaces it may be filled. Keep the mark monochrome plus the
single lime accent — never introduce indigo into the logo itself, so it stays legible at small
sizes and the two-accent system stays legible as "one hero color, one support color."

---

## 7. Applying the System Across Digi Guru

| Surface | Base mode | Notes |
|---|---|---|
| Marketing site (`/`) | Dark hero (§1.3 row 1) | Use the reference composition directly: headline + two-tone highlight, stat row, primary + ghost CTA pair, portrait with glow |
| Auth (`/login`, `/signup`) | Dark, split layout | Hero copy + brand on one side (reusing marketing hero styling), form card (`ink-900`) on the other — first impression should feel continuous with the marketing site |
| Student dashboard | Light mode default | `lavender-50` background, `lime-500` primary actions; a student browsing courses is in "planning" mode, not "focus" mode |
| Classroom (live session) | **Dark, always** — regardless of the student's dashboard preference | This is the "focus mode" surface. The whiteboard sits on `ink-950`; annotation/highlight strokes use `lime-400`; the "AI is listening/speaking" indicator uses an `indigo-400` pulsing ring (reusing the portrait-glow gradient at small scale) — see `.claude/rules/whiteboard-sync.md` for the technical protocol this styles |
| Admin console (super-admin / sub-admin) | Light mode | Data-dense CRUD surfaces; `lime-500` for primary actions (Save, Upload), `indigo-400` for informational badges (role tags, status pills), `danger`/`warning` reserved for destructive/quota states |
| Quota / safety states | Functional colors only | 18-minute warning → `warning`; hard 20-minute teardown message → neutral `ink-900` card, no red (it's a graceful stop, not an error); jailbreak-termination incident → `danger`, admin-only surface |

**Rule of thumb:** the classroom is the one surface that is *always* dark, independent of any
user-level light/dark preference — it's the product's signature moment and should look identical
to the marketing hero's mood every time a student enters it.

---

## 7.1 Responsive / Mobile Compatibility

Every surface in §7 must work down to a 360px-wide phone viewport, not just degrade gracefully —
mobile is the primary device for most students, not a fallback.

### Breakpoints (Tailwind defaults, used as-is — no custom scale)

| Token | Min width | Applies to |
|---|---|---|
| *(base, no prefix)* | 0px | Phone portrait — the default, unprefixed styles. Design mobile-first: write the phone layout as the base rule, then override up. |
| `sm:` | 640px | Phone landscape / small tablet |
| `md:` | 768px | Tablet portrait |
| `lg:` | 1024px | Tablet landscape / small laptop — marketing nav switches from hamburger to horizontal here |
| `xl:` | 1280px | Desktop — the compositions in §7 (hero + portrait glow, floating badges) reach full size here |

### Typography scale-down

The desktop type scale in §2 is too large on a 360–414px viewport. Fluid-scale the two display
tokens with `clamp()`; everything `heading-lg` and below is unchanged (already sized for mobile):

| Token | Mobile (base) | Desktop (`lg:` and up) |
|---|---|---|
| `display-lg` | `clamp(32px, 9vw, 56px)` / 1.1 | 56px / 1.05 |
| `display-md` | `clamp(26px, 6vw, 40px)` / 1.15 | 40px / 1.1 |

The two-tone headline rule (§2) still applies at every size — the emphasized word/phrase never
wraps onto its own line if avoidable; keep it attached to an adjacent word with `&nbsp;` if a break
would isolate it.

### Layout rules by surface

- **Marketing hero (`/`)**: single-column, stacked (headline → stat row → CTA pair → portrait) below
  `lg:`; the side-by-side headline/portrait composition only assembles at `lg:` and above. Floating
  decorative badges (§4) are hidden below `md:` — they compete with limited width and add no
  information.
- **Nav (§5.3)**: a hamburger/menu icon (`lime-400` on dark) replaces the horizontal link row below
  `lg:`, opening a full-height dark overlay panel (`ink-950` at 96% opacity) with the same links
  stacked at `heading-md` size, plus the CTA pair pinned to the bottom of the panel.
- **Stat row (§5.2)**: 3-across on desktop becomes a horizontal scroll-snap row on mobile (not a
  vertical stack — stacking triples the hero's height) — `overflow-x-auto` with `scroll-snap-type: x
  mandatory`, no visible scrollbar, each stat card `snap-align: start`.
- **Cards/grids (feature lists, testimonials, admin tables)**: `1` column below `sm:`, `2` at `sm:`,
  `3`+ at `lg:`. Never rely on `auto-fit`/`auto-fill` grid alone without an explicit mobile
  column-count override — those can produce a single very-wide column on a narrow viewport that
  looks broken rather than intentional.
- **Classroom (live session)**: the whiteboard canvas is the dominant element at every size; below
  `lg:`, the transcript/controls that would sit beside it on desktop move below it in document order,
  never overlapping the canvas.
- **Buttons/touch targets**: minimum 44×44px hit area on any breakpoint below `lg:` (WCAG 2.5.5 /
  iOS HIG), even where the visual button is smaller — pad with invisible touch area if needed, don't
  shrink the visible pill below `radius-full` proportions to hit a target size.

### Images & fonts on mobile

- Portrait/hero imagery (§4) uses `srcset`/`sizes` so a phone never downloads the desktop-resolution
  asset — generate at minimum 640px and 1280px widths.
- Font loading (§9.3): Malayalam subset loads only on curriculum routes regardless of viewport — this
  rule is about route, not device, and already minimizes mobile payload as a side effect.

---

## 8. Accessibility Notes

- `lime-400` on `white` fails WCAG AA for text ≤ 18px — restrict pure `lime-400` to large
  headline text, icons, and button fills with **dark** text on top. For lime *text* on a light
  background, use `lime-500`'s darker sibling or fall back to `gray-900` and reserve lime for the
  fill/icon only.
- `gray-300` on `ink-950` passes AA for body text (~9:1); do not go lighter-muted than `gray-300`
  for any text meant to be read, not just glanced at.
- Every interactive element gets a visible focus ring (`indigo-300`, 2px, 2px offset) — color
  alone never carries a state change (hover, active, error).
- Never use `lime-400` as the sole indicator of a safety/error state — pair with an icon and text,
  and use `danger`/`warning` for those states regardless of brand color pressure.

---

## 9. Developer Handoff

### 9.1 CSS custom properties

```css
:root {
  /* ink */
  --dg-ink-950: #0A0C16;
  --dg-ink-900: #12141F;
  --dg-ink-800: #1C1F2E;
  --dg-ink-700: #2A2E42;

  /* lime */
  --dg-lime-300: #C7F26B;
  --dg-lime-400: #A6E635;
  --dg-lime-500: #8FC91E;
  --dg-lime-950: #12190A;

  /* indigo */
  --dg-indigo-300: #9EA3F2;
  --dg-indigo-400: #6E74E8;
  --dg-indigo-500: #5A5FD1;
  --dg-indigo-950: #171A33;

  /* lavender / light surfaces */
  --dg-lavender-50: #F4F4FC;
  --dg-lavender-100: #E4E5F8;
  --dg-lavender-200: #D2D3F0;

  /* neutrals */
  --dg-white: #FFFFFF;
  --dg-gray-300: #B6B9C9;
  --dg-gray-500: #6C6F82;
  --dg-gray-900: #15161F;

  /* functional */
  --dg-success: #3FCF7F;
  --dg-danger: #F1503D;
  --dg-warning: #F5B84A;

  /* radius */
  --dg-radius-sm: 8px;
  --dg-radius-md: 16px;
  --dg-radius-lg: 28px;
  --dg-radius-full: 999px;
}
```

### 9.2 Tailwind theme

`apps/web` is on Tailwind v4, which reads theme tokens from CSS, not a JS config — this is the
actual block in `apps/web/src/app/globals.css`. (A `tailwind.config.js` `theme.extend` object with
the same values is the equivalent for a v3 project, if one is ever needed.)

```css
@theme {
  --color-ink-950: #0a0c16;
  --color-ink-900: #12141f;
  --color-ink-800: #1c1f2e;
  --color-ink-700: #2a2e42;

  --color-lime-300: #c7f26b;
  --color-lime-400: #a6e635;
  --color-lime-500: #8fc91e;
  --color-lime-950: #12190a;

  --color-indigo-300: #9ea3f2;
  --color-indigo-400: #6e74e8;
  --color-indigo-500: #5a5fd1;
  --color-indigo-950: #171a33;

  --color-lavender-50: #f4f4fc;
  --color-lavender-100: #e4e5f8;
  --color-lavender-200: #d2d3f0;

  /* previously missing from this section — the CSS vars in §9.1 already had
     these, but this Tailwind mapping didn't expose them, so `gray-*` classes
     silently fell back to Tailwind's stock gray scale (different hex values) */
  --color-gray-300: #b6b9c9;
  --color-gray-500: #6c6f82;
  --color-gray-900: #15161f;

  --color-success: #3fcf7f;
  --color-danger: #f1503d;
  --color-warning: #f5b84a;

  --font-display: "Sora", sans-serif;
  --font-sans: "Inter", sans-serif;
  --font-malayalam: "Noto Sans Malayalam", sans-serif;

  --radius-sm: 8px;
  --radius-md: 16px;
  --radius-lg: 28px;
}
```

### 9.3 Fonts

Google Fonts, self-hosted subset for production: `Sora:wght@600;700`, `Inter:wght@400;500;600`,
`Noto+Sans+Malayalam:wght@400;500;600;700`. Load Malayalam only on routes that render curriculum
content — no need to ship it on the marketing site.

---

## 10. Source

Palette and component patterns extracted from the approved landing-page hero mockup (dark hero,
lime + indigo accents, floating device frame). Hex values are close approximations read off that
comp — treat them as the working spec; replace with exact values if original design-tool files
(Figma) become available.
