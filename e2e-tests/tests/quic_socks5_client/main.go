package main

import (
	"context"
	"crypto/tls"
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/quic-go/quic-go"
	"github.com/quic-go/quic-go/http3"
)

var (
	socks5Host = getEnv("SOCKS5_HOST", "127.0.0.1")
	socks5Port = getEnvInt("SOCKS5_PORT", 1080)
	targetHost = getEnv("TARGET_HOST", "edith.xiaohongshu.com")
	targetPort = getEnvInt("TARGET_PORT", 443)
	targetPath = getEnv("TARGET_PATH", "/api/im/redmoji/version")
	testMode   = getEnv("TEST_MODE", "full")
)

func getEnv(key, defaultVal string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return defaultVal
}

func getEnvInt(key string, defaultVal int) int {
	if val := os.Getenv(key); val != "" {
		if i, err := strconv.Atoi(val); err == nil {
			return i
		}
	}
	return defaultVal
}

type Socks5UDPConn struct {
	tcpConn   net.Conn
	udpConn   *net.UDPConn
	relayAddr *net.UDPAddr
	localAddr net.Addr
}

func NewSocks5UDPConn(socks5Host string, socks5Port int) (*Socks5UDPConn, error) {
	tcpConn, err := net.DialTimeout("tcp", fmt.Sprintf("%s:%d", socks5Host, socks5Port), 10*time.Second)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to SOCKS5 server: %w", err)
	}

	if _, err := tcpConn.Write([]byte{0x05, 0x01, 0x00}); err != nil {
		tcpConn.Close()
		return nil, fmt.Errorf("failed to send auth request: %w", err)
	}

	authResp := make([]byte, 2)
	if _, err := io.ReadFull(tcpConn, authResp); err != nil {
		tcpConn.Close()
		return nil, fmt.Errorf("failed to read auth response: %w", err)
	}
	if authResp[0] != 0x05 || authResp[1] != 0x00 {
		tcpConn.Close()
		return nil, fmt.Errorf("SOCKS5 auth failed: %x", authResp)
	}
	fmt.Println("✓ SOCKS5 auth OK")

	if _, err := tcpConn.Write([]byte{0x05, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00}); err != nil {
		tcpConn.Close()
		return nil, fmt.Errorf("failed to send UDP ASSOCIATE request: %w", err)
	}

	resp := make([]byte, 10)
	if _, err := io.ReadFull(tcpConn, resp); err != nil {
		tcpConn.Close()
		return nil, fmt.Errorf("failed to read UDP ASSOCIATE response: %w", err)
	}

	if resp[1] != 0x00 {
		tcpConn.Close()
		return nil, fmt.Errorf("UDP ASSOCIATE failed: reply=%d", resp[1])
	}

	var relayIP net.IP
	var relayPort uint16

	switch resp[3] {
	case 0x01:
		relayIP = net.IP(resp[4:8])
		relayPort = binary.BigEndian.Uint16(resp[8:10])
	default:
		tcpConn.Close()
		return nil, fmt.Errorf("unsupported address type: %d", resp[3])
	}

	if relayIP.Equal(net.IPv4zero) {
		relayIP = net.ParseIP(socks5Host)
	}

	relayAddr := &net.UDPAddr{IP: relayIP, Port: int(relayPort)}
	fmt.Printf("✓ UDP relay at %s\n", relayAddr)

	udpConn, err := net.ListenUDP("udp", nil)
	if err != nil {
		tcpConn.Close()
		return nil, fmt.Errorf("failed to create UDP socket: %w", err)
	}

	return &Socks5UDPConn{
		tcpConn:   tcpConn,
		udpConn:   udpConn,
		relayAddr: relayAddr,
		localAddr: udpConn.LocalAddr(),
	}, nil
}

func (c *Socks5UDPConn) wrapPacket(data []byte, destHost string, destPort int) []byte {
	header := []byte{0x00, 0x00, 0x00}

	ip := net.ParseIP(destHost)
	if ip != nil && ip.To4() != nil {
		header = append(header, 0x01)
		header = append(header, ip.To4()...)
	} else {
		header = append(header, 0x03)
		header = append(header, byte(len(destHost)))
		header = append(header, []byte(destHost)...)
	}

	portBytes := make([]byte, 2)
	binary.BigEndian.PutUint16(portBytes, uint16(destPort))
	header = append(header, portBytes...)

	return append(header, data...)
}

func (c *Socks5UDPConn) unwrapPacket(data []byte) ([]byte, string, int) {
	if len(data) < 10 {
		return data, "", 0
	}

	atyp := data[3]
	var addr string
	var port int
	var payloadStart int

	switch atyp {
	case 0x01:
		addr = net.IP(data[4:8]).String()
		port = int(binary.BigEndian.Uint16(data[8:10]))
		payloadStart = 10
	case 0x03:
		addrLen := int(data[4])
		addr = string(data[5 : 5+addrLen])
		port = int(binary.BigEndian.Uint16(data[5+addrLen : 7+addrLen]))
		payloadStart = 7 + addrLen
	case 0x04:
		addr = net.IP(data[4:20]).String()
		port = int(binary.BigEndian.Uint16(data[20:22]))
		payloadStart = 22
	default:
		return data, "", 0
	}

	return data[payloadStart:], addr, port
}

func (c *Socks5UDPConn) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	buf := make([]byte, 65535)
	n, _, err = c.udpConn.ReadFromUDP(buf)
	if err != nil {
		return 0, nil, err
	}

	payload, host, port := c.unwrapPacket(buf[:n])
	copy(p, payload)

	if host != "" {
		addr = &net.UDPAddr{IP: net.ParseIP(host), Port: port}
	}

	return len(payload), addr, nil
}

func (c *Socks5UDPConn) WriteTo(p []byte, addr net.Addr) (n int, err error) {
	udpAddr, ok := addr.(*net.UDPAddr)
	if !ok {
		return 0, fmt.Errorf("invalid address type")
	}

	packet := c.wrapPacket(p, udpAddr.IP.String(), udpAddr.Port)
	_, err = c.udpConn.WriteToUDP(packet, c.relayAddr)
	if err != nil {
		return 0, err
	}

	return len(p), nil
}

func (c *Socks5UDPConn) LocalAddr() net.Addr {
	return c.localAddr
}

func (c *Socks5UDPConn) SetDeadline(t time.Time) error {
	return c.udpConn.SetDeadline(t)
}

func (c *Socks5UDPConn) SetReadDeadline(t time.Time) error {
	return c.udpConn.SetReadDeadline(t)
}

func (c *Socks5UDPConn) SetWriteDeadline(t time.Time) error {
	return c.udpConn.SetWriteDeadline(t)
}

func (c *Socks5UDPConn) Close() error {
	c.udpConn.Close()
	return c.tcpConn.Close()
}

type Socks5Transport struct {
	socks5Host string
	socks5Port int
}

func (t *Socks5Transport) Dial(ctx context.Context, addr string, tlsConf *tls.Config, quicConf *quic.Config) (quic.EarlyConnection, error) {
	host, portStr, err := net.SplitHostPort(addr)
	if err != nil {
		return nil, err
	}
	port, _ := strconv.Atoi(portStr)

	socks5Conn, err := NewSocks5UDPConn(t.socks5Host, t.socks5Port)
	if err != nil {
		return nil, err
	}

	targetIPs, err := net.LookupIP(host)
	if err != nil {
		socks5Conn.Close()
		return nil, err
	}

	var targetIP net.IP
	for _, ip := range targetIPs {
		if ip.To4() != nil {
			targetIP = ip
			break
		}
	}
	if targetIP == nil {
		socks5Conn.Close()
		return nil, fmt.Errorf("no IPv4 address found")
	}

	remoteAddr := &net.UDPAddr{IP: targetIP, Port: port}
	return quic.DialEarly(ctx, socks5Conn, remoteAddr, tlsConf, quicConf)
}

func testConnectionOnly() {
	fmt.Println("============================================================")
	fmt.Println("  HTTP/3 (QUIC) via SOCKS5 UDP Test - Connection Only")
	fmt.Println("============================================================")
	fmt.Printf("Target: https://%s%s\n", targetHost, targetPath)
	fmt.Printf("SOCKS5: %s:%d\n\n", socks5Host, socks5Port)

	socks5Conn, err := NewSocks5UDPConn(socks5Host, socks5Port)
	if err != nil {
		fmt.Printf("❌ Failed to setup SOCKS5 UDP: %v\n", err)
		os.Exit(1)
	}
	defer socks5Conn.Close()

	targetIPs, err := net.LookupIP(targetHost)
	if err != nil {
		fmt.Printf("❌ DNS resolution failed: %v\n", err)
		os.Exit(1)
	}
	var targetIP net.IP
	for _, ip := range targetIPs {
		if ip.To4() != nil {
			targetIP = ip
			break
		}
	}
	if targetIP == nil {
		fmt.Printf("❌ No IPv4 address found for %s\n", targetHost)
		os.Exit(1)
	}
	fmt.Printf("✓ Resolved %s -> %s\n\n", targetHost, targetIP)

	fmt.Println("Establishing QUIC connection via SOCKS5 UDP...")

	tlsConfig := &tls.Config{
		InsecureSkipVerify: true,
		NextProtos:         []string{"h3"},
		ServerName:         targetHost,
	}

	quicConfig := &quic.Config{
		MaxIdleTimeout:  30 * time.Second,
		KeepAlivePeriod: 10 * time.Second,
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	remoteAddr := &net.UDPAddr{IP: targetIP, Port: targetPort}

	conn, err := quic.Dial(ctx, socks5Conn, remoteAddr, tlsConfig, quicConfig)
	if err != nil {
		fmt.Printf("❌ QUIC connection failed: %v\n", err)
		os.Exit(1)
	}
	defer conn.CloseWithError(0, "done")

	fmt.Println("✅ QUIC connection established via SOCKS5 UDP!")
	fmt.Printf("   Protocol: %s\n", conn.ConnectionState().TLS.NegotiatedProtocol)
	fmt.Println("============================================================")
	fmt.Println("✅ QUIC connection via SOCKS5 UDP test PASSED!")
}

func testFullHTTP3() {
	fmt.Println("============================================================")
	fmt.Println("  HTTP/3 (QUIC) via SOCKS5 UDP Test - Full HTTP/3 Request")
	fmt.Println("============================================================")
	fmt.Printf("Target: https://%s%s\n", targetHost, targetPath)
	fmt.Printf("SOCKS5: %s:%d\n\n", socks5Host, socks5Port)

	transport := &Socks5Transport{
		socks5Host: socks5Host,
		socks5Port: socks5Port,
	}

	roundTripper := &http3.RoundTripper{
		TLSClientConfig: &tls.Config{
			InsecureSkipVerify: true,
			ServerName:         targetHost,
		},
		QUICConfig: &quic.Config{
			MaxIdleTimeout:  30 * time.Second,
			KeepAlivePeriod: 10 * time.Second,
		},
		Dial: transport.Dial,
	}
	defer roundTripper.Close()

	client := &http.Client{
		Transport: roundTripper,
		Timeout:   30 * time.Second,
	}

	url := fmt.Sprintf("https://%s%s", targetHost, targetPath)
	fmt.Printf("Sending HTTP/3 request to %s...\n\n", url)

	resp, err := client.Get(url)
	if err != nil {
		fmt.Printf("❌ HTTP/3 request failed: %v\n", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read response: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("============================================================")
	fmt.Println("  HTTP/3 Response")
	fmt.Println("============================================================")
	fmt.Printf("Status: %s\n", resp.Status)
	fmt.Printf("Protocol: %s\n", resp.Proto)
	fmt.Printf("Content-Type: %s\n", resp.Header.Get("Content-Type"))
	fmt.Printf("Body: %s\n", string(body))
	fmt.Println("============================================================")

	if resp.StatusCode == 200 && strings.Contains(string(body), "success") {
		fmt.Println("✅ HTTP/3 via SOCKS5 UDP test PASSED!")
	} else {
		fmt.Printf("⚠ Unexpected response: status=%d\n", resp.StatusCode)
	}
}

func main() {
	switch testMode {
	case "connection":
		testConnectionOnly()
	case "full":
		testFullHTTP3()
	default:
		testFullHTTP3()
	}
}
