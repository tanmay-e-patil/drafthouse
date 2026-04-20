export default function Footer() {
  const year = new Date().getFullYear()

  return (
    <footer className="footer">
      <div className="footer-inner">
        <p>&copy; {year} Drafthouse</p>
      </div>
    </footer>
  )
}
