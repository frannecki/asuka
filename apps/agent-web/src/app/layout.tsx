import type { Metadata } from "next";
import Link from "next/link";
import { Baloo_2, IBM_Plex_Mono, Manrope } from "next/font/google";

import "./globals.css";

const baloo = Baloo_2({
  variable: "--font-display",
  subsets: ["latin"],
  weight: ["600", "700"],
});

const manrope = Manrope({
  variable: "--font-body",
  subsets: ["latin"],
});

const plexMono = IBM_Plex_Mono({
  variable: "--font-mono",
  subsets: ["latin"],
  weight: ["400", "500"],
});

export const metadata: Metadata = {
  title: "Asuka Agent Workspace",
  description: "Local agent harness with session workspaces and streamed runs",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${baloo.variable} ${manrope.variable} ${plexMono.variable}`}>
        <div className="page-splash" aria-hidden="true" />
        <div className="page-halftone" aria-hidden="true" />

        <div className="app-shell app-shell-simple">
          <header className="global-nav">
            <Link className="nav-brand" href="/">
              <span className="nav-brand-icon">A</span>
              <div>
                <strong>Asuka</strong>
                <span>chat workspace</span>
              </div>
            </Link>

            <nav className="nav-links" aria-label="Primary">
              <Link className="nav-link-pill" href="/">
                Home
              </Link>
              <Link className="nav-link-pill" href="/dashboard">
                Dashboard
              </Link>
            </nav>
          </header>

          <main className="app-main">{children}</main>
        </div>
      </body>
    </html>
  );
}
