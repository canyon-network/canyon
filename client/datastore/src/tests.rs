// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Canyon.
//
// Copyright (c) 2021 Canyon Labs.
//
// Canyon is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published
// by the Free Software Foundation, either version 3 of the License,
// or (at your option) any later version.
//
// Canyon is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Canyon. If not, see <http://www.gnu.org/licenses/>.

use std::sync::Arc;

use sp_keystore::testing::KeyStore;
use substrate_test_runtime_client::DefaultTestClientBuilderExt;
use substrate_test_runtime_client::TestClientBuilderExt;

use cp_permastore::PermaStorage;

use crate::PermanentStorage;

#[test]
fn basic_operations_should_work() {
    let keystore = Arc::new(KeyStore::new());
    let client_builder = substrate_test_runtime_client::TestClientBuilder::new();
    let client = Arc::new(client_builder.set_keystore(keystore.clone()).build());

    let mut perma_storage = PermanentStorage::new_test(client.clone());

    perma_storage.submit(b"key", b"value");

    assert!(perma_storage.exists(b"key"));
    assert_eq!(perma_storage.retrieve(b"key"), Some(b"value".to_vec()));

    perma_storage.remove(b"key");
    assert!(!perma_storage.exists(b"key"));
    assert_eq!(perma_storage.retrieve(b"key"), None);
}
