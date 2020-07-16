mod support;

use libsignal_protocol_rust::*;
use rand::rngs::OsRng;
use std::convert::TryFrom;

fn encrypt(
    store: &mut InMemSignalProtocolStore,
    remote_address: &ProtocolAddress,
    msg: &str,
) -> Result<CiphertextMessage, SignalProtocolError> {
    let mut session_cipher = SessionCipher::new(
        remote_address.clone(),
        &mut store.session_store,
        &mut store.identity_store,
        &mut store.signed_pre_key_store,
        &mut store.pre_key_store,
    );
    session_cipher.encrypt(msg.as_bytes())
}

fn decrypt(
    store: &mut InMemSignalProtocolStore,
    remote_address: &ProtocolAddress,
    msg: &CiphertextMessage,
) -> Result<Vec<u8>, SignalProtocolError> {
    let mut session_cipher = SessionCipher::new(
        remote_address.clone(),
        &mut store.session_store,
        &mut store.identity_store,
        &mut store.signed_pre_key_store,
        &mut store.pre_key_store,
    );
    session_cipher.decrypt(msg)
}

#[test]
fn test_basic_prekey_v3() -> Result<(), SignalProtocolError> {
    let mut csprng = OsRng;

    let alice_address = ProtocolAddress::new("+14151111111".to_owned(), 1);
    let bob_address = ProtocolAddress::new("+14151111112".to_owned(), 1);

    let mut alice_store = support::test_in_memory_protocol_store();
    let mut bob_store = support::test_in_memory_protocol_store();

    let bob_pre_key_pair = KeyPair::new(&mut csprng);
    let bob_signed_pre_key_pair = KeyPair::new(&mut csprng);

    let bob_signed_pre_key_public = bob_signed_pre_key_pair.public_key.serialize();
    let bob_signed_pre_key_signature = bob_store
        .get_identity_key_pair()?
        .private_key()
        .calculate_signature(&bob_signed_pre_key_public, &mut csprng)?;

    let pre_key_id = 31337;
    let signed_pre_key_id = 22;

    let bob_pre_key_bundle = PreKeyBundle::new(
        bob_store.get_local_registration_id()?,
        1,                                 // device id
        Some(pre_key_id),                  // pre key id
        Some(bob_pre_key_pair.public_key), // pre key
        signed_pre_key_id,                 // signed pre key id
        bob_signed_pre_key_pair.public_key,
        bob_signed_pre_key_signature.to_vec(),
        *bob_store.get_identity_key_pair()?.identity_key(),
    )?;

    process_prekey_bundle(
        &bob_address,
        &mut alice_store.session_store,
        &mut alice_store.identity_store,
        &bob_pre_key_bundle,
        &mut csprng,
    )?;

    assert!(alice_store.contains_session(&bob_address)?);
    assert_eq!(
        alice_store
            .load_session(&bob_address)?
            .unwrap()
            .session_state()?
            .session_version()?,
        3
    );

    let original_message = "L'homme est condamné à être libre";

    let outgoing_message = encrypt(&mut alice_store, &bob_address, original_message)?;

    assert_eq!(
        outgoing_message.message_type(),
        CiphertextMessageType::PreKey
    );

    let incoming_message = CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::try_from(
        outgoing_message.serialize(),
    )?);

    bob_store.save_pre_key(
        pre_key_id,
        &PreKeyRecord::new(pre_key_id, &bob_pre_key_pair),
    )?;
    bob_store.save_signed_pre_key(
        signed_pre_key_id,
        &SignedPreKeyRecord::new(
            signed_pre_key_id,
            /*timestamp*/ 42,
            &bob_signed_pre_key_pair,
            &bob_signed_pre_key_signature,
        ),
    )?;

    let ptext = decrypt(&mut bob_store, &alice_address, &incoming_message)?;

    assert_eq!(String::from_utf8(ptext).unwrap(), original_message);

    let bobs_response = "Who watches the watchers?";

    assert!(bob_store.contains_session(&alice_address)?);
    let bobs_session_with_alice = bob_store.load_session(&alice_address)?.unwrap();
    assert_eq!(
        bobs_session_with_alice.session_state()?.session_version()?,
        3
    );
    assert_eq!(
        bobs_session_with_alice
            .session_state()?
            .alice_base_key()?
            .len(),
        32 + 1
    );

    let bob_outgoing = encrypt(&mut bob_store, &alice_address, bobs_response)?;

    assert_eq!(bob_outgoing.message_type(), CiphertextMessageType::Whisper);

    let alice_decrypts = decrypt(&mut alice_store, &bob_address, &bob_outgoing)?;

    assert_eq!(String::from_utf8(alice_decrypts).unwrap(), bobs_response);

    run_interaction(
        &mut alice_store,
        &alice_address,
        &mut bob_store,
        &bob_address,
    )?;

    let mut alice_store = support::test_in_memory_protocol_store();

    let bob_pre_key_pair = KeyPair::new(&mut csprng);
    let bob_signed_pre_key_pair = KeyPair::new(&mut csprng);

    let bob_signed_pre_key_public = bob_signed_pre_key_pair.public_key.serialize();
    let bob_signed_pre_key_signature = bob_store
        .get_identity_key_pair()?
        .private_key()
        .calculate_signature(&bob_signed_pre_key_public, &mut csprng)?;

    let pre_key_id = 31337;
    let signed_pre_key_id = 22;

    let bob_pre_key_bundle = PreKeyBundle::new(
        bob_store.get_local_registration_id()?,
        1, // device id
        Some(pre_key_id + 1),
        Some(bob_pre_key_pair.public_key), // pre key
        signed_pre_key_id + 1,
        bob_signed_pre_key_pair.public_key,
        bob_signed_pre_key_signature.to_vec(),
        *bob_store.get_identity_key_pair()?.identity_key(),
    )?;

    bob_store.save_pre_key(
        pre_key_id + 1,
        &PreKeyRecord::new(pre_key_id + 1, &bob_pre_key_pair),
    )?;
    bob_store.save_signed_pre_key(
        signed_pre_key_id + 1,
        &SignedPreKeyRecord::new(
            signed_pre_key_id + 1,
            /*timestamp*/ 42,
            &bob_signed_pre_key_pair,
            &bob_signed_pre_key_signature,
        ),
    )?;

    process_prekey_bundle(
        &bob_address,
        &mut alice_store.session_store,
        &mut alice_store.identity_store,
        &bob_pre_key_bundle,
        &mut csprng,
    )?;

    let outgoing_message = encrypt(&mut alice_store, &bob_address, original_message)?;

    assert_eq!(
        decrypt(&mut bob_store, &alice_address, &outgoing_message).unwrap_err(),
        SignalProtocolError::UntrustedIdentity(alice_address.clone())
    );

    assert_eq!(
        bob_store.save_identity(
            &alice_address,
            alice_store.get_identity_key_pair()?.identity_key()
        )?,
        true
    );

    let decrypted = decrypt(&mut bob_store, &alice_address, &outgoing_message)?;
    assert_eq!(String::from_utf8(decrypted).unwrap(), original_message);

    // Sign pre-key with wrong key:
    let bob_pre_key_bundle = PreKeyBundle::new(
        bob_store.get_local_registration_id()?,
        1, // device id
        Some(pre_key_id),
        Some(bob_pre_key_pair.public_key), // pre key
        signed_pre_key_id,
        bob_signed_pre_key_pair.public_key,
        bob_signed_pre_key_signature.to_vec(),
        *alice_store.get_identity_key_pair()?.identity_key(),
    )?;

    assert!(process_prekey_bundle(
        &bob_address,
        &mut alice_store.session_store,
        &mut alice_store.identity_store,
        &bob_pre_key_bundle,
        &mut csprng
    )
    .is_err());

    Ok(())
}

fn run_interaction(
    alice_store: &mut InMemSignalProtocolStore,
    alice_address: &ProtocolAddress,
    bob_store: &mut InMemSignalProtocolStore,
    bob_address: &ProtocolAddress,
) -> Result<(), SignalProtocolError> {
    let alice_ptext = "It's rabbit season";

    let alice_message = encrypt(alice_store, bob_address, alice_ptext)?;
    assert_eq!(alice_message.message_type(), CiphertextMessageType::Whisper);
    assert_eq!(
        String::from_utf8(decrypt(bob_store, alice_address, &alice_message)?).unwrap(),
        alice_ptext
    );

    let bob_ptext = "It's duck season";

    let bob_message = encrypt(bob_store, alice_address, bob_ptext)?;
    assert_eq!(bob_message.message_type(), CiphertextMessageType::Whisper);
    assert_eq!(
        String::from_utf8(decrypt(alice_store, bob_address, &bob_message)?).unwrap(),
        bob_ptext
    );

    for i in 0..10 {
        let alice_ptext = format!("A->B message {}", i);
        let alice_message = encrypt(alice_store, bob_address, &alice_ptext)?;
        assert_eq!(alice_message.message_type(), CiphertextMessageType::Whisper);
        assert_eq!(
            String::from_utf8(decrypt(bob_store, alice_address, &alice_message)?).unwrap(),
            alice_ptext
        );
    }

    for i in 0..10 {
        let bob_ptext = format!("B->A message {}", i);
        let bob_message = encrypt(bob_store, alice_address, &bob_ptext)?;
        assert_eq!(bob_message.message_type(), CiphertextMessageType::Whisper);
        assert_eq!(
            String::from_utf8(decrypt(alice_store, bob_address, &bob_message)?).unwrap(),
            bob_ptext
        );
    }

    let mut alice_ooo_messages = vec![];

    for i in 0..10 {
        let alice_ptext = format!("A->B OOO message {}", i);
        let alice_message = encrypt(alice_store, bob_address, &alice_ptext)?;
        alice_ooo_messages.push((alice_ptext, alice_message));
    }

    for i in 0..10 {
        let alice_ptext = format!("A->B post-OOO message {}", i);
        let alice_message = encrypt(alice_store, bob_address, &alice_ptext)?;
        assert_eq!(alice_message.message_type(), CiphertextMessageType::Whisper);
        assert_eq!(
            String::from_utf8(decrypt(bob_store, alice_address, &alice_message)?).unwrap(),
            alice_ptext
        );
    }

    for i in 0..10 {
        let bob_ptext = format!("B->A message post-OOO {}", i);
        let bob_message = encrypt(bob_store, alice_address, &bob_ptext)?;
        assert_eq!(bob_message.message_type(), CiphertextMessageType::Whisper);
        assert_eq!(
            String::from_utf8(decrypt(alice_store, bob_address, &bob_message)?).unwrap(),
            bob_ptext
        );
    }

    for (ptext, ctext) in alice_ooo_messages {
        assert_eq!(
            String::from_utf8(decrypt(bob_store, alice_address, &ctext)?).unwrap(),
            ptext
        );
    }

    Ok(())
}

/*

  private void runInteraction(SignalProtocolStore aliceStore, SignalProtocolStore bobStore)
      throws DuplicateMessageException, LegacyMessageException, InvalidMessageException, NoSessionException, UntrustedIdentityException
  {
    Set<Pair<String, CiphertextMessage>> aliceOutOfOrderMessages = new HashSet<>();

    for (int i=0;i<10;i++) {
      String loopingMessage = ("What do we mean by saying that existence precedes essence? " +
                               "We mean that man first of all exists, encounters himself, " +
                               "surges up in the world--and defines himself aftward. " + i);
      CiphertextMessage aliceLoopingMessage = aliceSessionCipher.encrypt(loopingMessage.getBytes());

      aliceOutOfOrderMessages.add(new Pair<>(loopingMessage, aliceLoopingMessage));
    }

    for (int i=0;i<10;i++) {
      String loopingMessage = ("What do we mean by saying that existence precedes essence? " +
                               "We mean that man first of all exists, encounters himself, " +
                               "surges up in the world--and defines himself aftward. " + i);
      CiphertextMessage aliceLoopingMessage = aliceSessionCipher.encrypt(loopingMessage.getBytes());

      byte[] loopingPlaintext = bobSessionCipher.decrypt(new SignalMessage(aliceLoopingMessage.serialize()));
      assertTrue(new String(loopingPlaintext).equals(loopingMessage));
    }

    for (int i=0;i<10;i++) {
      String loopingMessage = ("You can only desire based on what you know: " + i);
      CiphertextMessage bobLoopingMessage = bobSessionCipher.encrypt(loopingMessage.getBytes());

      byte[] loopingPlaintext = aliceSessionCipher.decrypt(new SignalMessage(bobLoopingMessage.serialize()));
      assertTrue(new String(loopingPlaintext).equals(loopingMessage));
    }

    for (Pair<String, CiphertextMessage> aliceOutOfOrderMessage : aliceOutOfOrderMessages) {
      byte[] outOfOrderPlaintext = bobSessionCipher.decrypt(new SignalMessage(aliceOutOfOrderMessage.second().serialize()));
      assertTrue(new String(outOfOrderPlaintext).equals(aliceOutOfOrderMessage.first()));
    }
  }

*/