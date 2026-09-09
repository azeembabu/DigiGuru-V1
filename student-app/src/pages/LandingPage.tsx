import { Link } from 'react-router-dom';

export default function LandingPage() {
  return (
    <div className="min-h-screen bg-nature">
      {/* Navigation */}
      <header className="relative z-10 mx-auto flex max-w-7xl items-center justify-between px-4 py-5 sm:px-6 lg:px-8">
        <Link to="/" className="flex items-center gap-2.5 focus-ring rounded-xl">
          <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-dg-900 text-white text-[17px] font-bold shadow-lg" aria-hidden>
            ◆
          </span>
          <span className="text-[19px] font-semibold tracking-tight text-ink-900">Digi Guru</span>
        </Link>

        <nav className="hidden items-center gap-8 md:flex">
          <a href="#features" className="text-sm font-medium text-ink-700 hover:text-ink-900 transition">Features</a>
          <a href="#how-it-works" className="text-sm font-medium text-ink-700 hover:text-ink-900 transition">How It Works</a>
          <a href="#testimonials" className="text-sm font-medium text-ink-700 hover:text-ink-900 transition">Testimonials</a>
        </nav>

        <div className="flex items-center gap-3">
          <Link
            to="/login"
            className="hidden sm:inline-flex items-center justify-center rounded-full border border-slate-200 bg-white/90 px-5 py-2.5 text-sm font-medium text-ink-700 backdrop-blur transition hover:bg-white focus-ring"
          >
            Log in
          </Link>
          <Link
            to="/signup"
            className="inline-flex items-center justify-center rounded-full bg-dg-900 px-6 py-2.5 text-sm font-medium text-white shadow-[0_4px_16px_rgba(14,75,58,0.25)] transition hover:bg-dg-950 focus-ring"
          >
            Get Started
          </Link>
        </div>
      </header>

      {/* Hero Section */}
      <div className="relative z-10 mx-auto max-w-7xl px-4 pb-16 pt-12 sm:px-6 sm:pb-24 sm:pt-20 lg:px-8">
        <div className="text-center">
          <p className="font-display text-[11px] font-medium uppercase tracking-[0.20em] text-dg-700 mb-4">
            AI-Powered Learning Platform
          </p>
          <h1 className="font-display text-[42px] font-normal leading-[1.1] text-ink-900 sm:text-[52px] lg:text-[62px]">
            Your personal AI teacher,
            <br />
            <span className="font-medium text-dg-900">teaching from your textbook</span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-base leading-relaxed text-ink-500 sm:text-lg">
            Learn with a real-like AI teacher powered by Gemini. Paragraph-by-paragraph explanations on a whiteboard,
            with voice, notes, flashcards, and oral exams — all from your own textbooks.
          </p>

          <div className="mt-10 flex flex-col items-center gap-4 sm:flex-row sm:justify-center">
            <Link
              to="/signup"
              className="inline-flex items-center justify-center rounded-full bg-dg-900 px-8 py-3.5 text-base font-medium text-white shadow-[0_4px_20px_rgba(14,75,58,0.30)] transition hover:bg-dg-950 focus-ring"
            >
              Start Learning Free
            </Link>
            <Link
              to="/login"
              className="inline-flex items-center justify-center rounded-full border-2 border-dg-200 bg-white/80 px-8 py-3.5 text-base font-medium text-dg-900 backdrop-blur transition hover:bg-white hover:border-dg-300 focus-ring"
            >
              I already have an account
            </Link>
          </div>

          {/* Stats */}
          <div className="mt-12 flex flex-wrap items-center justify-center gap-8 sm:gap-12">
            <div className="text-center">
              <p className="text-3xl font-semibold text-dg-900">500+</p>
              <p className="mt-1 text-sm text-ink-500">Active Students</p>
            </div>
            <div className="text-center">
              <p className="text-3xl font-semibold text-dg-900">12+</p>
              <p className="mt-1 text-sm text-ink-500">Programs</p>
            </div>
            <div className="text-center">
              <p className="text-3xl font-semibold text-dg-900">98%</p>
              <p className="mt-1 text-sm text-ink-500">Satisfaction Rate</p>
            </div>
          </div>
        </div>

        {/* Testimonial Avatars */}
        <div className="mt-12 flex items-center justify-center gap-2">
          <div className="flex -space-x-2">
            {[1, 2, 3, 4, 5].map((i) => (
              <div
                key={i}
                className="h-10 w-10 rounded-full border-2 border-white bg-gradient-to-br from-dg-200 to-dg-300 flex items-center justify-center text-sm font-medium text-dg-900"
              >
                {String.fromCharCode(64 + i)}
              </div>
            ))}
          </div>
          <p className="ml-3 text-sm text-ink-600">
            <span className="font-semibold text-ink-900">30,000+</span> learners worldwide
          </p>
        </div>
      </div>

      {/* Features Section */}
      <div id="features" className="relative z-10 mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
        <div className="mb-12 text-center">
          <p className="font-display text-[11px] font-medium uppercase tracking-[0.18em] text-dg-700">Features</p>
          <h2 className="mt-3 font-display text-[32px] font-normal text-ink-900 sm:text-[38px]">
            Everything you need to <span className="font-medium text-dg-900">learn effectively</span>
          </h2>
        </div>

        <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {[
            {
              icon: '🎙️',
              title: 'Voice-Powered Teaching',
              description: 'Speak your doubts and hear the AI teacher explain concepts in natural voice.',
            },
            {
              icon: '📖',
              title: 'Textbook-Based Learning',
              description: 'AI teaches only from your uploaded textbooks — paragraph by paragraph, page by page.',
            },
            {
              icon: '📝',
              title: 'Auto-Generated Notes',
              description: 'Get comprehensive notes, flashcards, and mind maps after every class.',
            },
            {
              icon: '🎯',
              title: 'Adaptive Learning',
              description: 'AI adjusts teaching speed and complexity based on your understanding level.',
            },
            {
              icon: '📊',
              title: 'Whiteboard Sessions',
              description: 'Visual explanations on an interactive whiteboard with real-time drawings.',
            },
            {
              icon: '✅',
              title: 'Oral Exams & Tests',
              description: 'Take voice-based oral exams and get instant feedback with mistake highlighting.',
            },
          ].map((feature, i) => (
            <div
              key={i}
              className="glass-card-strong rounded-2xl p-6 transition hover:shadow-lg"
            >
              <div className="mb-3 text-3xl">{feature.icon}</div>
              <h3 className="text-lg font-semibold text-ink-900">{feature.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-ink-500">{feature.description}</p>
            </div>
          ))}
        </div>
      </div>

      {/* How It Works */}
      <div id="how-it-works" className="relative z-10 mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
        <div className="mb-12 text-center">
          <p className="font-display text-[11px] font-medium uppercase tracking-[0.18em] text-dg-700">How It Works</p>
          <h2 className="mt-3 font-display text-[32px] font-normal text-ink-900 sm:text-[38px]">
            Start learning in <span className="font-medium text-dg-900">3 simple steps</span>
          </h2>
        </div>

        <div className="grid gap-8 sm:grid-cols-3">
          {[
            {
              step: '01',
              title: 'Create Your Account',
              description: 'Sign up with your Roll Number, Program, Semester, and LSC. Takes 30 seconds.',
            },
            {
              step: '02',
              title: 'Access Your Textbooks',
              description: 'Your AI teacher has access to your program textbooks — ready to teach.',
            },
            {
              step: '03',
              title: 'Start Learning',
              description: 'Join a 20-minute class with voice, whiteboard, and get auto-generated study materials.',
            },
          ].map((item, i) => (
            <div key={i} className="text-center">
              <div className="mb-4 inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-dg-900 text-xl font-bold text-white">
                {item.step}
              </div>
              <h3 className="text-lg font-semibold text-ink-900">{item.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-ink-500">{item.description}</p>
            </div>
          ))}
        </div>

        <div className="mt-12 text-center">
          <Link
            to="/signup"
            className="inline-flex items-center justify-center rounded-full bg-dg-900 px-8 py-3.5 text-base font-medium text-white shadow-[0_4px_20px_rgba(14,75,58,0.30)] transition hover:bg-dg-950 focus-ring"
          >
            Create Your Account Now
          </Link>
        </div>
      </div>

      {/* Testimonials */}
      <div id="testimonials" className="relative z-10 mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
        <div className="mb-12 text-center">
          <p className="font-display text-[11px] font-medium uppercase tracking-[0.18em] text-dg-700">Testimonials</p>
          <h2 className="mt-3 font-display text-[32px] font-normal text-ink-900 sm:text-[38px]">
            What our learners <span className="font-medium text-dg-900">say about us</span>
          </h2>
        </div>

        <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {[
            {
              name: 'Arun Krishna',
              program: 'BA Malayalam, Semester 1',
              quote: 'The AI teacher explains concepts just like a real teacher. I can ask doubts anytime and get instant clarification with voice and whiteboard.',
              avatar: 'A',
            },
            {
              name: 'Meera Nair',
              program: 'BA English, Semester 2',
              quote: 'The auto-generated notes and flashcards save me hours of study time. The oral exams are incredibly helpful for exam preparation.',
              avatar: 'M',
            },
            {
              name: 'Rahul Menon',
              program: 'BCom Finance, Semester 3',
              quote: 'I love how the AI adapts to my learning pace. When I don\'t understand something, it explains again patiently. Best learning platform!',
              avatar: 'R',
            },
          ].map((testimonial, i) => (
            <div key={i} className="glass-card-strong rounded-2xl p-6">
              <div className="mb-4 flex items-center gap-3">
                <div className="flex h-12 w-12 items-center justify-center rounded-full bg-gradient-to-br from-dg-200 to-dg-300 text-lg font-semibold text-dg-900">
                  {testimonial.avatar}
                </div>
                <div>
                  <p className="font-semibold text-ink-900">{testimonial.name}</p>
                  <p className="text-xs text-ink-500">{testimonial.program}</p>
                </div>
              </div>
              <p className="text-sm leading-relaxed text-ink-600">"{testimonial.quote}"</p>
            </div>
          ))}
        </div>
      </div>

      {/* Partner Logos */}
      <div className="relative z-10 mx-auto max-w-7xl px-4 py-12 sm:px-6 lg:px-8">
        <p className="mb-6 text-center text-xs font-medium uppercase tracking-wider text-ink-400">
          Trusted by learners across Kerala
        </p>
        <div className="flex flex-wrap items-center justify-center gap-8 opacity-60 grayscale">
          {['LSC Palakkad', 'LSC Thrissur', 'LSC Malappuram', 'BA Programs', 'BCom Programs'].map((partner, i) => (
            <div key={i} className="rounded-lg bg-white/80 px-4 py-2 text-sm font-medium text-ink-600 backdrop-blur">
              {partner}
            </div>
          ))}
        </div>
      </div>

      {/* Final CTA */}
      <div className="relative z-10 mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8">
        <div className="glass-card-strong rounded-3xl p-8 text-center sm:p-12">
          <h2 className="font-display text-[28px] font-normal text-ink-900 sm:text-[34px]">
            Ready to start your <span className="font-medium text-dg-900">learning journey</span>?
          </h2>
          <p className="mx-auto mt-4 max-w-xl text-base text-ink-500">
            Join thousands of students learning with their personal AI teacher. No credit card required.
          </p>
          <div className="mt-8 flex flex-col items-center gap-4 sm:flex-row sm:justify-center">
            <Link
              to="/signup"
              className="inline-flex items-center justify-center rounded-full bg-dg-900 px-8 py-3.5 text-base font-medium text-white shadow-[0_4px_20px_rgba(14,75,58,0.30)] transition hover:bg-dg-950 focus-ring"
            >
              Get Started for Free
            </Link>
            <Link
              to="/login"
              className="inline-flex items-center justify-center rounded-full border-2 border-dg-200 bg-white/80 px-8 py-3.5 text-base font-medium text-dg-900 backdrop-blur transition hover:bg-white hover:border-dg-300 focus-ring"
            >
              Log In
            </Link>
          </div>
        </div>
      </div>

      {/* Footer */}
      <footer className="relative z-10 mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
        <div className="border-t border-slate-200 pt-8">
          <div className="flex flex-col items-center justify-between gap-4 sm:flex-row">
            <div className="flex items-center gap-2">
              <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-dg-900 text-white text-[13px] font-bold" aria-hidden>
                ◆
              </span>
              <span className="text-sm font-semibold text-ink-900">Digi Guru</span>
            </div>
            <p className="text-xs text-ink-400">
              © 2026 Digi Guru. All rights reserved.
            </p>
            <div className="flex gap-6">
              <a href="#" className="text-xs text-ink-500 hover:text-ink-700 transition">Privacy</a>
              <a href="#" className="text-xs text-ink-500 hover:text-ink-700 transition">Terms</a>
              <a href="#" className="text-xs text-ink-500 hover:text-ink-700 transition">Support</a>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}
