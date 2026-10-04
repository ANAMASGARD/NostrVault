package main

import (
	"filippo.io/age"
	"os"
)

func main() {
	if len(os.Args) != 3 {
		panic("expected payload ZIP and output directory")
	}
	payload, e := os.ReadFile(os.Args[1])
	if e != nil {
		panic(e)
	}
	identity, e := age.GenerateX25519Identity()
	if e != nil {
		panic(e)
	}
	if e = os.WriteFile(os.Args[2]+"/age-identity.txt", []byte(identity.String()+"\n"), 0600); e != nil {
		panic(e)
	}
	if e = os.WriteFile(os.Args[2]+"/age-recipient.txt", []byte(identity.Recipient().String()+"\n"), 0644); e != nil {
		panic(e)
	}
	pass, e := age.NewScryptRecipient("NostrVault public fixture password, never a user secret")
	if e != nil {
		panic(e)
	}
	pass.SetWorkFactor(16)
	for name, recipient := range map[string]age.Recipient{"recipient": identity.Recipient(), "passphrase": pass} {
		f, e := os.Create(os.Args[2] + "/reference-" + name + ".age")
		if e != nil {
			panic(e)
		}
		w, e := age.Encrypt(f, recipient)
		if e != nil {
			panic(e)
		}
		if _, e = w.Write(payload); e != nil {
			panic(e)
		}
		if e = w.Close(); e != nil {
			panic(e)
		}
		if e = f.Close(); e != nil {
			panic(e)
		}
	}
}
