import type { Metadata } from "next";
import Link from "next/link";
import { IBM_Plex_Mono, Space_Grotesk } from "next/font/google";

import "./globals.css";

const spaceGrotesk = Space_Grotesk({
  variable: "--font-display",
  subsets: ["latin"],
});

const plexMono = IBM_Plex_Mono({
  variable: "--font-mono",
  subsets: ["latin"],
  weight: ["400", "500"],
});

export const metadata: Metadata = {
  title: "Asuka Agent Console",
  description: "Deep-agent backend and chat console prototype",
};

const navigation = [
  { href: "/", label: "Overview" },
  { href: "/chat", label: "Chat" },
  { href: "/memory", label: "Memory" },
  { href: "/skills", label: "Skills" },
  { href: "/subagents", label: "Subagents" },
  { href: "/settings/providers", label: "Providers" },
  { href: "/settings/mcp", label: "MCP" },
];

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${spaceGrotesk.variable} ${plexMono.variable}`}>
        <div className="app-shell">
          <header className="app-header">
            <div>
              <p className="eyebrow">Asuka</p>
              <h1>Deep Agent Console</h1>
            </div>
            <nav className="top-nav">
              {navigation.map((item) => (
                <Link href={item.href} key={item.href}>
                  {item.label}
                </Link>
              ))}
            </nav>
          </header>

          <main className="app-main">{children}</main>
        </div>
      </body>
    </html>
  );
}
