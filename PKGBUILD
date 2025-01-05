# Maintainer: Voislav Vasiljevski <voyo@thelazysre.com>
pkgname=observer
pkgver=0.1.0
pkgrel=1
pkgdesc="Dynamic CPU core manager for power saving"
arch=('x86_64')
url="https://github.com/voioo/observer"
license=('MIT')
depends=('gcc-libs')
makedepends=('rust' 'cargo')
backup=('etc/observer/config')
source=("$pkgname-$pkgver.tar.gz::$url/releases/download/v$pkgver/$pkgname-linux-amd64.tar.gz")
sha256sums=('SKIP')

package() {
    cd "$srcdir"
    install -Dm755 "$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm644 "$pkgname.service" "$pkgdir/usr/lib/systemd/system/$pkgname.service"
    install -Dm644 "config.toml" "$pkgdir/etc/$pkgname/config"
}